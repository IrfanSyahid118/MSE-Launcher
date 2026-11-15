use crate::state::AppState;
use launcher_types::LauncherErrorPayload;
use serde_json::json;
use tauri::Emitter;

#[tauri::command]
pub async fn downloads_cleanup(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, LauncherErrorPayload> {
    let task_id = uuid::Uuid::new_v4().to_string();
    let task_id_clone = task_id.clone();
    let paths = state.paths.clone();

    tokio::spawn(async move {
        let paths_for_task = paths.clone();
        match tokio::task::spawn_blocking(move || clean_temp_and_cache(&paths_for_task)).await {
            Ok((removed_files, freed_bytes)) => {
                let _ = app.emit(
                    "task:done",
                    json!({
                        "taskId": task_id_clone,
                        "result": {
                            "removedFiles": removed_files,
                            "freedBytes": freed_bytes,
                            "warnings": Vec::<String>::new(),
                        }
                    }),
                );
            }
            Err(join_err) => {
                let err = LauncherErrorPayload::file_system(format!(
                    "Pembersihan cache gagal dijalankan: {}",
                    join_err
                ));
                let _ = app.emit(
                    "task:error",
                    super::task_error_payload(&task_id_clone, &err),
                );
            }
        }
    });

    Ok(json!({ "taskId": task_id }))
}

fn clean_dir_entries(dir: &std::path::Path, removed_files: &mut usize, freed_bytes: &mut u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let p = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_symlink() {
            let _ = std::fs::remove_dir(&p).or_else(|_| std::fs::remove_file(&p));
        } else if ft.is_dir() {
            clean_dir_entries(&p, removed_files, freed_bytes);
            let _ = std::fs::remove_dir(&p);
        } else {
            if ft.is_file() {
                *freed_bytes += meta.len();
            }
            if std::fs::remove_file(&p).is_ok() {
                *removed_files += 1;
            }
        }
    }
}

fn clean_temp_and_cache(paths: &launcher_platform::DataPaths) -> (usize, u64) {
    let mut removed_files = 0;
    let mut freed_bytes = 0;

    let dirs_to_clean = [paths.root.join("temp"), paths.cache_dir.clone()];
    for dir in &dirs_to_clean {
        let Ok(meta) = std::fs::symlink_metadata(dir) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
            continue;
        }
        if paths.root.exists()
            && !launcher_security::canonical_starts_with(dir, &paths.root).unwrap_or(false)
        {
            continue;
        }
        clean_dir_entries(dir, &mut removed_files, &mut freed_bytes);
    }

    (removed_files, freed_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_n25_downloads_cleanup_removes_subdirectories() {
        let root = std::env::temp_dir().join(format!("mcl-n25-{}", uuid::Uuid::new_v4()));
        let paths = launcher_platform::DataPaths::new(&root);
        let temp_sub = paths.root.join("temp").join("extract-123");
        let cache_sub = paths.cache_dir.join("modpack-staging").join("nested");
        std::fs::create_dir_all(&temp_sub).unwrap();
        std::fs::create_dir_all(&cache_sub).unwrap();

        let top_file = paths.root.join("temp").join("top.tmp");
        let sub_file1 = temp_sub.join("a.bin");
        let sub_file2 = cache_sub.join("b.jar");
        std::fs::write(&top_file, b"1234").unwrap();
        std::fs::write(&sub_file1, b"123456").unwrap();
        std::fs::write(&sub_file2, b"12345678").unwrap();

        let (removed, freed) = clean_temp_and_cache(&paths);
        let temp_sub_exists = temp_sub.exists();
        let cache_staging_exists = paths.cache_dir.join("modpack-staging").exists();
        let root_temp_exists = paths.root.join("temp").exists();
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(removed, 3, "should remove top-level and nested files");
        assert_eq!(freed, 4 + 6 + 8);
        assert!(!temp_sub_exists, "temp subdirectory must be removed");
        assert!(
            !cache_staging_exists,
            "cache modpack staging subdirectory must be removed"
        );
        assert!(
            root_temp_exists,
            "top-level temp directory itself should remain"
        );
    }

    #[cfg(windows)]
    #[test]
    fn test_ps16_ps17_clean_temp_and_cache_does_not_traverse_junction() {
        let base = std::env::temp_dir().join(format!("mcl-ps16-{}", uuid::Uuid::new_v4()));
        let root = base.join("root");
        let outside = base.join("outside-keep");
        let keep_file = outside.join("precious.txt");
        let paths = launcher_platform::DataPaths::new(&root);
        let junction = paths.root.join("temp").join("linked");
        std::fs::create_dir_all(paths.root.join("temp")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(&keep_file, b"keep").unwrap();

        let status = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                outside.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(status.success());

        let _ = clean_temp_and_cache(&paths);
        let kept = keep_file.exists();
        let _ = std::fs::remove_dir(&junction);
        let _ = std::fs::remove_dir_all(&base);
        assert!(
            kept,
            "clean_temp_and_cache must never delete files inside NTFS junction target!"
        );
    }
}
