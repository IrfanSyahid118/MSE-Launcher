use crate::state::AppState;
use launcher_security::validate_instance_id;
use launcher_types::{CreateInstanceRequest, Instance, LauncherErrorPayload};
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
