use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use zip::ZipArchive;

pub const PAYLOAD_MAGIC: [u8; 8] = *b"MSEINST1";
pub const FOOTER_LEN: u64 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PayloadManifest {
    pub version: String,
    pub app_exe_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PayloadError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("No embedded payload found in installer")]
    MissingPayload,
    #[error("Invalid embedded payload layout")]
    InvalidLayout,
}

struct SubReader {
    file: File,
    start: u64,
    len: u64,
    pos: u64,
}

impl SubReader {
    fn new(mut file: File, start: u64, end: u64) -> io::Result<Self> {
        if end < start {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid sub-reader range",
            ));
        }
        file.seek(SeekFrom::Start(start))?;
        Ok(Self {
            file,
            start,
            len: end - start,
            pos: 0,
        })
    }
}

impl Read for SubReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len {
            return Ok(0);
        }
        let remaining = (self.len - self.pos) as usize;
        let max_read = buf.len().min(remaining);
        let n = self.file.read(&mut buf[..max_read])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for SubReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::End(offset) => self.len as i128 + offset as i128,
            SeekFrom::Current(offset) => self.pos as i128 + offset as i128,
        };
        if next < 0 || next > self.len as i128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek outside payload bounds",
            ));
        }
        let next_u64 = next as u64;
        let abs_offset = self
            .start
            .checked_add(next_u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek offset overflow"))?;
        self.file.seek(SeekFrom::Start(abs_offset))?;
        self.pos = next_u64;
        Ok(self.pos)
    }
}

pub fn validate_app_exe_name(name: &str) -> Result<(), PayloadError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains('\0')
    {
        return Err(PayloadError::InvalidLayout);
    }
    Ok(())
}

pub fn find_payload_offset(exe_path: &Path) -> Result<u64, PayloadError> {
    let mut file = File::open(exe_path)?;
    let file_len = file.metadata()?.len();
    if file_len < FOOTER_LEN {
        return Err(PayloadError::MissingPayload);
    }

    file.seek(SeekFrom::Start(file_len - FOOTER_LEN))?;
    let mut footer = [0u8; 16];
    file.read_exact(&mut footer)?;

    if footer[8..16] != PAYLOAD_MAGIC {
        return Err(PayloadError::MissingPayload);
    }

    let offset = u64::from_le_bytes(
        footer[0..8]
            .try_into()
            .map_err(|_| PayloadError::InvalidLayout)?,
    );
    if offset == 0 || offset >= file_len - FOOTER_LEN {
        return Err(PayloadError::InvalidLayout);
    }
    Ok(offset)
}

pub fn read_payload_manifest(exe_path: &Path) -> Result<(u64, PayloadManifest), PayloadError> {
    let offset = find_payload_offset(exe_path)?;
    let file = File::open(exe_path)?;
    let file_len = file.metadata()?.len();
    let reader = SubReader::new(file, offset, file_len - FOOTER_LEN)?;
    let mut archive = ZipArchive::new(reader)?;
    let mut manifest_entry = archive.by_name("manifest.json")?;
    let mut buf = Vec::new();
    manifest_entry.read_to_end(&mut buf)?;
    let manifest: PayloadManifest = serde_json::from_slice(&buf)?;
    Ok((offset, manifest))
}

struct TempFileCleanup<'a>(&'a Path, bool);

impl Drop for TempFileCleanup<'_> {
    fn drop(&mut self) {
        if !self.1 {
            let _ = std::fs::remove_file(self.0);
        }
    }
}

fn replace_dest_with_tmp(tmp_path: &Path, dest_path: &Path) -> io::Result<()> {
    let backup_path = dest_path.with_extension("exe.bak");
    let had_existing = dest_path.exists();
    if had_existing {
        let _ = std::fs::remove_file(&backup_path);
        std::fs::rename(dest_path, &backup_path)?;
    }

    match std::fs::rename(tmp_path, dest_path) {
        Ok(()) => {
            if had_existing {
                let _ = std::fs::remove_file(&backup_path);
            }
            Ok(())
        }
        Err(err) => {
            if had_existing {
                let _ = std::fs::rename(&backup_path, dest_path);
            }
            Err(err)
        }
    }
}

pub fn extract_payload_app(
    exe_path: &Path,
    payload_offset: u64,
    app_exe_name: &str,
    dest_path: &Path,
) -> Result<u64, PayloadError> {
    validate_app_exe_name(app_exe_name)?;

    let file = File::open(exe_path)?;
    let file_len = file.metadata()?.len();
    let reader = SubReader::new(file, payload_offset, file_len - FOOTER_LEN)?;
    let mut archive = ZipArchive::new(reader)?;
    let mut entry = archive.by_name(app_exe_name)?;
    let total_size = entry.size();

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp_path = dest_path.with_extension("exe.tmp");
    let mut guard = TempFileCleanup(&tmp_path, false);
    {
        let mut out = File::create(&tmp_path)?;
        io::copy(&mut entry, &mut out)?;
        out.flush()?;
    }

    replace_dest_with_tmp(&tmp_path, dest_path)?;
    guard.1 = true;
    Ok(total_size)
}

pub fn copy_stub_without_payload(
    exe_path: &Path,
    payload_offset: u64,
    dest_path: &Path,
) -> Result<(), PayloadError> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut src = File::open(exe_path)?;
    let tmp_path = dest_path.with_extension("exe.tmp");
    let mut guard = TempFileCleanup(&tmp_path, false);
    {
        let mut out = File::create(&tmp_path)?;
        let mut limited = io::Read::by_ref(&mut src).take(payload_offset);
        io::copy(&mut limited, &mut out)?;
        out.flush()?;
    }

    replace_dest_with_tmp(&tmp_path, dest_path)?;
    guard.1 = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn build_fake_installer(dir: &Path) -> std::path::PathBuf {
        let installer_path = dir.join("setup.exe");
        let stub_bytes = b"MZ_FAKE_STUB_BINARY_CONTENT_123456";
        let mut zip_buf = Vec::new();
        {
            let cursor = io::Cursor::new(&mut zip_buf);
            let mut writer = ZipWriter::new(cursor);
            writer
                .start_file("manifest.json", SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(br#"{"version":"1.2.0-beta","appExeName":"MSE Launcher.exe"}"#)
                .unwrap();
            writer
                .start_file("MSE Launcher.exe", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"APP_PAYLOAD_BYTES").unwrap();
            writer.finish().unwrap();
        }

        let offset = stub_bytes.len() as u64;
        let mut out = File::create(&installer_path).unwrap();
        out.write_all(stub_bytes).unwrap();
        out.write_all(&zip_buf).unwrap();
        out.write_all(&offset.to_le_bytes()).unwrap();
        out.write_all(&PAYLOAD_MAGIC).unwrap();
        out.flush().unwrap();
        installer_path
    }

    #[test]
    fn test_payload_roundtrip_and_stub_split() {
        let tmp = tempfile::tempdir().unwrap();
        let setup_exe = build_fake_installer(tmp.path());

        let (offset, manifest) = read_payload_manifest(&setup_exe).unwrap();
        assert_eq!(manifest.version, "1.2.0-beta");
        assert_eq!(manifest.app_exe_name, "MSE Launcher.exe");

        let app_dest = tmp.path().join("install").join("MSE Launcher.exe");
        let extracted_size =
            extract_payload_app(&setup_exe, offset, &manifest.app_exe_name, &app_dest).unwrap();
        assert_eq!(extracted_size, 17);
        assert_eq!(std::fs::read(&app_dest).unwrap(), b"APP_PAYLOAD_BYTES");

        let uninstall_dest = tmp.path().join("install").join("uninstall.exe");
        copy_stub_without_payload(&setup_exe, offset, &uninstall_dest).unwrap();
        assert_eq!(
            std::fs::read(&uninstall_dest).unwrap(),
            b"MZ_FAKE_STUB_BINARY_CONTENT_123456"
        );
    }

    #[test]
    fn test_validate_app_exe_name_and_subreader_bounds() {
        assert!(validate_app_exe_name("MSE Launcher.exe").is_ok());
        assert!(validate_app_exe_name("../evil.exe").is_err());
        assert!(validate_app_exe_name("sub/evil.exe").is_err());
        assert!(validate_app_exe_name("C:evil.exe").is_err());
        assert!(validate_app_exe_name("").is_err());

        let tmp = tempfile::tempdir().unwrap();
        let setup_exe = build_fake_installer(tmp.path());
        let file = File::open(&setup_exe).unwrap();
        let mut reader = SubReader::new(file, 10, 20).unwrap();
        assert!(reader.seek(SeekFrom::Start(11)).is_err());
        assert!(reader.seek(SeekFrom::Start(u64::MAX)).is_err());
        assert!(reader.seek(SeekFrom::End(1)).is_err());
        assert_eq!(reader.seek(SeekFrom::End(0)).unwrap(), 10);
    }
}
