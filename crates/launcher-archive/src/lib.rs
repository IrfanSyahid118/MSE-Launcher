pub mod resourcepack;

use launcher_types::LauncherErrorPayload;
pub use resourcepack::*;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub const MAX_ARCHIVE_ENTRIES: usize = 50_000;
pub const MAX_UNCOMPRESSED_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// Mengekstrak seluruh atau sebagian berkas ZIP / JAR ke direktori tujuan.
/// Memiliki proteksi penuh terhadap kerentanan Zip-Slip (SEC-01) dan Zip Bomb (SEC-05).
pub fn extract_zip(
    archive_path: &Path,
    destination: &Path,
    exclusions: &[&str],
) -> Result<Vec<PathBuf>, LauncherErrorPayload> {
    extract_zip_with_limit(
        archive_path,
        destination,
        exclusions,
        MAX_UNCOMPRESSED_BYTES,
    )
}

pub fn extract_zip_with_limit(
    archive_path: &Path,
    destination: &Path,
    exclusions: &[&str],
    max_bytes: u64,
) -> Result<Vec<PathBuf>, LauncherErrorPayload> {
    if !archive_path.exists() {
        return Err(LauncherErrorPayload::not_found(format!(
            "Berkas arsip '{}' tidak ditemukan.",
            archive_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("archive")
        )));
    }

    launcher_security::ensure_secure_dir(destination, false)?;

    if fs::symlink_metadata(destination)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(LauncherErrorPayload::invalid_input(
            "Direktori tujuan ekstraksi tidak boleh berupa symlink.",
        ));
    }

    let canonical_dest = dunce::canonicalize(destination).map_err(|e| {
        LauncherErrorPayload::file_system(format!(
            "Gagal melakukan kanonikalisasi direktori: {}",
            e
        ))
    })?;

    let file = File::open(archive_path).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal membuka berkas arsip: {}", e))
    })?;

    let mut archive = ZipArchive::new(file).map_err(|e| {
        LauncherErrorPayload::invalid_input(format!("Format ZIP tidak valid: {}", e))
    })?;

    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(LauncherErrorPayload::invalid_input(format!(
            "Jumlah entri ZIP ({} entri) melebihi batas keamanan maksimum {} entri (SEC-05).",
            archive.len(),
            MAX_ARCHIVE_ENTRIES
        )));
    }

    let mut extracted_paths = Vec::new();
    let mut seen_case_folded: HashSet<String> = HashSet::new();
    let mut total_uncompressed_bytes: u64 = 0;

    let res: Result<(), LauncherErrorPayload> = (|| {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| {
                LauncherErrorPayload::internal(format!("Gagal membaca entri arsip ke-{}: {}", i, e))
            })?;

            let raw_name = entry
                .name()
                .map_err(|e| {
                    LauncherErrorPayload::invalid_input(format!(
                        "Nama entri arsip ke-{} tidak valid (UTF-8): {}",
                        i, e
                    ))
                })?
                .to_string();

            if exclusions.iter().any(|ex| raw_name.starts_with(ex)) {
                continue;
            }

            if entry
                .unix_mode()
                .is_some_and(|mode| (mode & 0o170000) == 0o120000)
            {
                return Err(LauncherErrorPayload::invalid_input(format!(
                    "Entri ZIP bertipe symlink ditolak: '{}'",
                    raw_name
                )));
            }

            if raw_name.contains('\\') || raw_name.contains('\0') {
                return Err(LauncherErrorPayload::invalid_input(format!(
                    "Entri ZIP memuat karakter separator/NUL terlarang: '{}'",
                    raw_name
                )));
            }

            let is_dir_entry = entry.is_dir();
            let trimmed_rel = raw_name.trim_end_matches('/');
            if trimmed_rel.is_empty() && is_dir_entry {
                continue;
            }

            let enclosed_name = entry.enclosed_name().ok_or_else(|| {
                LauncherErrorPayload::invalid_input(format!(
                    "Entri zip berbahaya terdeteksi (Zip-Slip attempt): '{}'",
                    raw_name
                ))
            })?;

            let safe_rel = launcher_security::SafeRelPath::parse(trimmed_rel).map_err(|e| {
                LauncherErrorPayload::invalid_input(format!(
                    "Nama entri ZIP tidak memenuhi kebijakan keamanan ({}): '{}'",
                    e.message, raw_name
                ))
            })?;

            let folded_key = format!(
                "{}:{}",
                if is_dir_entry { "D" } else { "F" },
                trimmed_rel.to_ascii_lowercase()
            );
            if !is_dir_entry && !seen_case_folded.insert(folded_key) {
                return Err(LauncherErrorPayload::invalid_input(format!(
                    "Konflik duplikasi atau tabrakan huruf besar/kecil entri ZIP: '{}'",
                    raw_name
                )));
            }

            let _ = enclosed_name;
            let target_path = destination.join(safe_rel.as_path());
            launcher_security::ensure_descendants_no_symlink(destination, &target_path)?;

            if is_dir_entry {
                fs::create_dir_all(&target_path).map_err(|e| {
                    LauncherErrorPayload::file_system(format!("Gagal membuat folder: {}", e))
                })?;
                if !launcher_security::canonical_starts_with(&target_path, &canonical_dest)? {
                    return Err(LauncherErrorPayload::invalid_input(
                        "Pelanggaran isolasi direktori pada folder ZIP.",
                    ));
                }
            } else {
                let parent = target_path.parent().ok_or_else(|| {
                    LauncherErrorPayload::invalid_input(
                        "Path target tidak memiliki parent direktori.",
                    )
                })?;

                if !parent.exists() {
                    fs::create_dir_all(parent).map_err(|e| {
                        LauncherErrorPayload::file_system(format!(
                            "Gagal membuat folder parent: {}",
                            e
                        ))
                    })?;
                }

                launcher_security::ensure_descendants_no_symlink(destination, parent)?;
                if !launcher_security::canonical_starts_with(parent, &canonical_dest)? {
                    return Err(LauncherErrorPayload::invalid_input(
                        "Pelanggaran isolasi direktori: entri berada di luar direktori tujuan.",
                    ));
                }

                let remaining_bytes = max_bytes.saturating_sub(total_uncompressed_bytes);
                if remaining_bytes == 0 {
                    return Err(LauncherErrorPayload::invalid_input(
                        "Ukuran dekompresi arsip melebihi batas (Zip Bomb protection SEC-05).",
                    ));
                }

                if let Ok(meta) = fs::symlink_metadata(&target_path) {
                    if meta.file_type().is_symlink() || meta.file_type().is_dir() {
                        return Err(LauncherErrorPayload::invalid_input(
                            "Target file ekstraksi merupakan symlink atau direktori.",
                        ));
                    }
                    launcher_security::safe_remove_file_within(&canonical_dest, &target_path)?;
                }

                let mut out_file = launcher_security::safe_create_new_file(&target_path, false)?;

                let mut limited = (&mut entry).take(remaining_bytes.saturating_add(1));
                let copied = io::copy(&mut limited, &mut out_file).map_err(|e| {
                    LauncherErrorPayload::file_system(format!("Gagal mengekstrak isi entri: {}", e))
                })?;

                if copied > remaining_bytes {
                    let _ = fs::remove_file(&target_path);
                    return Err(LauncherErrorPayload::invalid_input(
                        "Ukuran dekompresi arsip melebihi batas (Zip Bomb protection SEC-05).",
                    ));
                }

                total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(copied);

                if !launcher_security::canonical_starts_with(&target_path, &canonical_dest)? {
                    let _ = fs::remove_file(&target_path);
                    return Err(LauncherErrorPayload::invalid_input(
                        "Pelanggaran isolasi berkas: berkas berada di luar direktori tujuan.",
                    ));
                }

                extracted_paths.push(target_path);
            }
        }
        Ok(())
    })();

    match res {
        Ok(()) => Ok(extracted_paths),
        Err(e) => {
            for path in &extracted_paths {
                let _ = fs::remove_file(path);
            }
            fn prune_empty_subdirs(dir: &Path, base: &Path) {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            prune_empty_subdirs(&p, base);
                        }
                    }
                }
                if dir != base {
                    let _ = fs::remove_dir(dir);
                }
            }
            prune_empty_subdirs(destination, destination);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn test_extract_zip_normal_and_exclusions() {
        let temp_dir = tempdir().unwrap();
        let zip_path = temp_dir.path().join("test.zip");
        let dest_dir = temp_dir.path().join("extracted");

        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();

            zip.start_file("hello.txt", options).unwrap();
            zip.write_all(b"Hello world!").unwrap();

            zip.start_file("META-INF/MANIFEST.MF", options).unwrap();
            zip.write_all(b"Manifest-Version: 1.0").unwrap();

            zip.finish().unwrap();
        }

        let extracted = extract_zip(&zip_path, &dest_dir, &["META-INF/"]).unwrap();

        assert_eq!(extracted.len(), 1);
        assert!(dest_dir.join("hello.txt").exists());
        assert!(!dest_dir.join("META-INF").exists());

        let content = fs::read_to_string(dest_dir.join("hello.txt")).unwrap();
        assert_eq!(content, "Hello world!");
    }

    #[test]
    fn test_zip_slip_rejected() {
        let temp_dir = tempdir().unwrap();
        let zip_path = temp_dir.path().join("slip.zip");
        let dest_dir = temp_dir.path().join("extracted");

        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();

            zip.start_file("../outside.txt", options).unwrap();
            zip.write_all(b"malicious").unwrap();
            zip.finish().unwrap();
        }

        let res = extract_zip(&zip_path, &dest_dir, &[]);
        assert!(res.is_err());
        assert!(!temp_dir.path().join("outside.txt").exists());
    }

    #[test]
    fn test_activate_resourcepack_empty_list_syntax() {
        let temp_dir = tempdir().unwrap();
        let options_file = temp_dir.path().join("options.txt");
        std::fs::write(&options_file, "resourcePacks:[]\n").unwrap();

        activate_resourcepack_in_options(&options_file, "SkinLoader-ResourcePack.zip").unwrap();

        let updated = std::fs::read_to_string(&options_file).unwrap();
        let pack_line = updated
            .lines()
            .find(|l| l.starts_with("resourcePacks:"))
            .unwrap();

        assert!(
            !pack_line.contains(",]"),
            "Invalid JSON in options.txt generated with trailing comma: '{}'",
            pack_line
        );
        assert!(pack_line.contains("\"file/SkinLoader-ResourcePack.zip\""));
    }

    #[test]
    fn test_zip_bomb_bounded_extraction() {
        let temp_dir = tempdir().unwrap();
        let zip_path = temp_dir.path().join("bomb.zip");
        let dest_dir = temp_dir.path().join("extracted");

        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();

            zip.start_file("huge.dat", options).unwrap();
            zip.write_all(&vec![b'A'; 100_000]).unwrap();
            zip.finish().unwrap();
        }

        let res = extract_zip_with_limit(&zip_path, &dest_dir, &[], 1_000);
        assert!(res.is_err(), "Zip melebihi batas harus gagal");
        assert!(
            !dest_dir.join("huge.dat").exists(),
            "Berkas parsial harus dibersihkan"
        );
    }

    #[test]
    fn test_zip_extraction_failure_cleans_empty_parents_bug22() {
        let temp_dir = tempdir().unwrap();
        let zip_path = temp_dir.path().join("nested_bomb.zip");
        let dest_dir = temp_dir.path().join("extracted_nested");

        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();

            zip.start_file("nested/deep/folder/huge.dat", options)
                .unwrap();
            zip.write_all(&vec![b'B'; 50_000]).unwrap();
            zip.finish().unwrap();
        }

        let res = extract_zip_with_limit(&zip_path, &dest_dir, &[], 500);
        assert!(res.is_err(), "Must fail due to limit");
        assert!(
            !dest_dir.join("nested/deep/folder").exists(),
            "Empty parent folders must be pruned on failure"
        );
    }

    #[test]
    fn test_ps06_zip_rejects_backslash_reserved_symlink_case_collision_and_preexisting_symlink() {
        {
            let temp_dir = tempdir().unwrap();
            let zip_path = temp_dir.path().join("backslash.zip");
            let dest_dir = temp_dir.path().join("out");
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.start_file("..\\escaped.dll", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
            assert!(extract_zip(&zip_path, &dest_dir, &[]).is_err());
            assert!(!temp_dir.path().join("escaped.dll").exists());
        }

        {
            let temp_dir = tempdir().unwrap();
            let zip_path = temp_dir.path().join("reserved.zip");
            let dest_dir = temp_dir.path().join("out");
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.start_file("natives/COM¹.dll", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
            assert!(extract_zip(&zip_path, &dest_dir, &[]).is_err());
        }

        {
            let temp_dir = tempdir().unwrap();
            let zip_path = temp_dir.path().join("symlink_mode.zip");
            let dest_dir = temp_dir.path().join("out");
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.add_symlink("evil_link", "/etc/passwd", SimpleFileOptions::default())
                .unwrap();
            zip.finish().unwrap();
            assert!(extract_zip(&zip_path, &dest_dir, &[]).is_err());
        }

        {
            let temp_dir = tempdir().unwrap();
            let zip_path = temp_dir.path().join("case_collision.zip");
            let dest_dir = temp_dir.path().join("out");
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.start_file("lib/Foo.so", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"one").unwrap();
            zip.start_file("lib/foo.so", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"two").unwrap();
            zip.finish().unwrap();
            assert!(extract_zip(&zip_path, &dest_dir, &[]).is_err());
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_ps06_zip_rejects_windows_junction_inside_dest() {
        let temp_dir = tempdir().unwrap();
        let outside = temp_dir.path().join("outside");
        let dest_dir = temp_dir.path().join("out");
        let sub_link = dest_dir.join("sub");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&dest_dir).unwrap();
        let status = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                sub_link.to_str().unwrap(),
                outside.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(status.success());

        let zip_path = temp_dir.path().join("write_through.zip");
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.start_file("sub/pwn.txt", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"pwned").unwrap();
            zip.finish().unwrap();
        }

        assert!(extract_zip(&zip_path, &dest_dir, &[]).is_err());
        assert!(!outside.join("pwn.txt").exists());
    }
}
