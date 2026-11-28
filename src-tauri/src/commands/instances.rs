use crate::state::AppState;
use launcher_security::validate_instance_id;
use launcher_types::{CreateInstanceRequest, Instance, InstanceScreenshot, LauncherErrorPayload};
use tauri_plugin_opener::OpenerExt;

const ALLOWED_SUBFOLDERS: &[&str] = &[
    "mods",
    "resourcepacks",
    "saves",
    "shaderpacks",
    "screenshots",
    "logs",
];

#[tauri::command]
pub fn instances_list(
    state: tauri::State<AppState>,
) -> Result<Vec<Instance>, LauncherErrorPayload> {
    state.instances.list()
}

#[tauri::command]
pub fn instances_create(
    state: tauri::State<AppState>,
    req: CreateInstanceRequest,
) -> Result<Instance, LauncherErrorPayload> {
    state.instances.create(req)
}

#[tauri::command]
pub fn instances_update(
    state: tauri::State<AppState>,
    req: Instance,
) -> Result<(), LauncherErrorPayload> {
    state.instances.update(req)
}

#[tauri::command]
pub fn instances_remove(
    state: tauri::State<AppState>,
    id: String,
) -> Result<(), LauncherErrorPayload> {
    state.instances.remove(&id)
}

#[tauri::command]
pub fn instances_open_folder(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    id: String,
    subfolder: Option<String>,
) -> Result<(), LauncherErrorPayload> {
    validate_instance_id(&id)?;

    let instance_dir = launcher_security::safe_subpath(&state.paths.instances_dir, &id)?;
    launcher_security::ensure_descendants_no_symlink(&state.paths.instances_dir, &instance_dir)?;
    if !instance_dir.exists() {
        return Err(LauncherErrorPayload::not_found(format!(
            "Folder instance '{}' tidak ditemukan.",
            id
        )));
    }

    let target_dir = if let Some(ref sub) = subfolder {
        if !ALLOWED_SUBFOLDERS.contains(&sub.as_str()) {
            return Err(LauncherErrorPayload::invalid_input(format!(
                "Subfolder '{}' tidak diizinkan.",
                sub
            )));
        }
        let dir = launcher_security::safe_subpath(&instance_dir, &format!("minecraft/{sub}"))?;
        launcher_security::ensure_dir_within(&instance_dir, &dir, false)?;
        dir
    } else {
        instance_dir
    };

    let target_str = target_dir.to_str().ok_or_else(|| {
        LauncherErrorPayload::invalid_input("Path folder instance bukan UTF-8 yang valid.")
    })?;

    app.opener()
        .open_path(target_str, None::<&str>)
        .map_err(|e| {
            LauncherErrorPayload::internal(format!("Gagal membuka folder di sistem operasi: {}", e))
        })?;

    Ok(())
}

fn collect_instance_screenshots(
    instances_dir: &std::path::Path,
    inst: &Instance,
    out: &mut Vec<(std::time::SystemTime, InstanceScreenshot)>,
) {
    if validate_instance_id(&inst.id).is_err() {
        return;
    }
    let Ok(inst_dir) = launcher_security::safe_subpath(instances_dir, &inst.id) else {
        return;
    };
    let screenshots_dir = inst_dir.join("minecraft").join("screenshots");
    let no_symlink =
        launcher_security::ensure_descendants_no_symlink(instances_dir, &screenshots_dir);
    if no_symlink.is_err() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&screenshots_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let lower = file_name.to_ascii_lowercase();
        if !lower.ends_with(".png") && !lower.ends_with(".jpg") && !lower.ends_with(".jpeg") {
            continue;
        }
        let path = entry.path();
        if launcher_security::ensure_descendants_no_symlink(&screenshots_dir, &path).is_err() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        let modified_at = chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339();
        out.push((
            modified,
            InstanceScreenshot {
                instance_id: inst.id.clone(),
                instance_name: inst.name.clone(),
                file_name,
                file_path: path.to_string_lossy().to_string(),
                size_bytes: meta.len(),
                modified_at,
            },
        ));
    }
}

#[tauri::command]
pub fn instances_list_screenshots(
    state: tauri::State<AppState>,
    instance_id: Option<String>,
) -> Result<Vec<InstanceScreenshot>, LauncherErrorPayload> {
    let target_instances: Vec<Instance> = if let Some(ref id) = instance_id {
        validate_instance_id(id)?;
        vec![state.instances.get(id)?]
    } else {
        state.instances.list()?
    };

    let mut collected: Vec<(std::time::SystemTime, InstanceScreenshot)> = Vec::new();
    for inst in &target_instances {
        collect_instance_screenshots(&state.paths.instances_dir, inst, &mut collected);
    }

    collected.sort_by_key(|a| std::cmp::Reverse(a.0));
    Ok(collected.into_iter().map(|(_, item)| item).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_instance_screenshots_filters_images_and_sorts_newest_first() {
        let root = std::env::temp_dir().join(format!("mcl-ss-{}", uuid::Uuid::new_v4()));
        let paths = launcher_platform::DataPaths::new(&root);
        let store = launcher_storage::InstancesStore::new(paths.clone());
        let inst = store
            .create(CreateInstanceRequest {
                name: "ShotTest".to_string(),
                minecraft_version: "1.21.4".to_string(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Vanilla,
                    version: None,
                },
            })
            .unwrap();

        let ss_dir = paths
            .instances_dir
            .join(&inst.id)
            .join("minecraft")
            .join("screenshots");
        std::fs::create_dir_all(&ss_dir).unwrap();
        std::fs::write(ss_dir.join("first.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        std::fs::write(ss_dir.join("ignore.txt"), b"not an image").unwrap();
        std::fs::write(ss_dir.join("second.jpg"), b"\xff\xd8\xff").unwrap();

        let mut collected = Vec::new();
        collect_instance_screenshots(&paths.instances_dir, &inst, &mut collected);
        assert_eq!(collected.len(), 2);
        assert!(collected.iter().all(|(_, s)| s.file_name != "ignore.txt"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
