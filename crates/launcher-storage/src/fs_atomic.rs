use chrono::Utc;
use launcher_types::LauncherErrorPayload;
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Menulis data serializable ke berkas secara atomik.
/// Menulis ke file sementara dengan ekstensi unik di folder yang sama, lalu me-rename secara atomik.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), LauncherErrorPayload> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            LauncherErrorPayload::internal(format!("Gagal membuat folder {:?}: {}", parent, e))
        })?;
    }

    let serialized = serde_json::to_string_pretty(value).map_err(|e| {
        LauncherErrorPayload::internal(format!("Gagal serialisasi JSON untuk {:?}: {}", path, e))
    })?;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp_name = format!(
        "{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        uuid::Uuid::new_v4().simple()
    );
    let temp_path = parent.join(temp_name);

    {
        use std::io::Write;
        if let Ok(meta) = fs::symlink_metadata(path) {
            if meta.file_type().is_symlink() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target berkas penulisan atomik tidak boleh berupa symlink.",
                ));
            }
        }
        let mut file = launcher_security::safe_create_new_file(&temp_path, false)?;

        let write_res = (|| -> std::io::Result<()> {
            file.write_all(serialized.as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            Ok(())
        })();

        if let Err(e) = write_res {
            let _ = fs::remove_file(&temp_path);
            return Err(LauncherErrorPayload::internal(format!(
                "Gagal menulis dan sync file sementara: {}",
                e
            )));
        }
    }

    if let Err(e) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(LauncherErrorPayload::internal(format!(
            "Gagal rename atomik {:?} ke {:?}: {}",
            temp_path, path, e
        )));
    }

    Ok(())
}

/// Mengarantina berkas yang korup/tidak valid dengan me-rename menjadi `*.corrupt-<timestamp>`.
pub fn quarantine_corrupt_file(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }

    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let corrupt_name = format!("{}.corrupt-{}", file_name, timestamp);
    let corrupt_path = path.with_file_name(corrupt_name);

    if fs::rename(path, &corrupt_path).is_ok() {
        tracing::warn!(
            "Berkas korup {:?} telah dikarantina ke {:?}",
            path,
            corrupt_path
        );
        Some(corrupt_path)
    } else {
        None
    }
}

/// Membaca file JSON.
/// Jika file tidak ada -> Ok(None).
/// Jika file rusak/gagal parse -> karantina file ke `*.corrupt-*`, catat warning, dan kembalikan Ok(None).
pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, LauncherErrorPayload> {
    if !path.exists() {
        return Ok(None);
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return Err(LauncherErrorPayload::internal(format!(
                "Gagal membaca berkas {:?}: {}",
                path, e
            )))
        }
    };

    match serde_json::from_str::<T>(&content) {
        Ok(val) => Ok(Some(val)),
        Err(e) => {
            tracing::warn!(
                "Gagal parsing JSON pada {:?}: {}. Mengarantina berkas.",
                path,
                e
            );
            quarantine_corrupt_file(path);
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[derive(Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
    struct DummyData {
        name: String,
        count: u32,
    }

    #[test]
    fn test_write_json_atomic_and_quarantine_bug16() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("config.json");

        let data = DummyData {
            name: "test".to_string(),
            count: 42,
        };

        write_json_atomic(&file_path, &data).expect("write atomic failed");
        assert!(file_path.exists());

        let read_back: Option<DummyData> = read_json_file(&file_path).expect("read failed");
        assert_eq!(read_back, Some(data));

        fs::write(&file_path, b"INVALID JSON {[[{").unwrap();
        let read_corrupt: Option<DummyData> =
            read_json_file(&file_path).expect("quarantine failed");
        assert_eq!(read_corrupt, None);
        assert!(
            !file_path.exists(),
            "Corrupt file should have been renamed/quarantined"
        );

        let entries = fs::read_dir(temp.path()).unwrap();
        let corrupt_found = entries.flatten().any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("config.json.corrupt-")
        });
        assert!(
            corrupt_found,
            "Quarantined file must exist with .corrupt- prefix"
        );
    }
}
