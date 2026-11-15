use crate::logs::{DiagnosticInstanceMeta, DiagnosticReportResult, LauncherLogLine};
use crate::state::AppState;
use launcher_security::validate_instance_id;
use launcher_types::LauncherErrorPayload;
use std::path::PathBuf;
use tauri_plugin_opener::OpenerExt;

const MAX_CLIENT_LOG_CHARS: usize = 2000;

#[tauri::command]
pub fn logs_launcher_history(
    state: tauri::State<AppState>,
) -> Result<Vec<LauncherLogLine>, LauncherErrorPayload> {
    Ok(state.logs.history())
}

#[tauri::command]
pub fn logs_append_client(
    state: tauri::State<AppState>,
    stream: String,
    line: String,
) -> Result<(), LauncherErrorPayload> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let clipped: String = trimmed.chars().take(MAX_CLIENT_LOG_CHARS).collect();
    state
        .logs
        .record_entry(&stream, format!("[UI] {}", clipped));
    Ok(())
}

#[tauri::command]
pub fn logs_clear_launcher(state: tauri::State<AppState>) -> Result<(), LauncherErrorPayload> {
    state.logs.clear_memory();
    Ok(())
}

#[tauri::command]
pub fn logs_create_diagnostic(
    state: tauri::State<AppState>,
    instance_id: Option<String>,
    game_lines: Option<Vec<LauncherLogLine>>,
    last_error: Option<String>,
) -> Result<DiagnosticReportResult, LauncherErrorPayload> {
    let mut instance_meta: Option<DiagnosticInstanceMeta> = None;
    let mut fallback_latest_log: Option<PathBuf> = None;

    if let Some(ref raw_id) = instance_id {
        if validate_instance_id(raw_id).is_ok() {
            if let Ok(inst) = state.instances.get(raw_id) {
                instance_meta = Some(DiagnosticInstanceMeta {
                    id: inst.id.clone(),
                    name: inst.name.clone(),
                    minecraft_version: inst.minecraft_version.clone(),
                    loader: format!("{:?}", inst.loader.loader_type).to_ascii_lowercase(),
                });
            }
            let latest = state
                .paths
                .instances_dir
                .join(raw_id)
                .join("minecraft")
                .join("logs")
                .join("latest.log");
            if launcher_security::ensure_descendants_no_symlink(&state.paths.instances_dir, &latest)
                .is_ok()
                && latest.exists()
            {
                fallback_latest_log = Some(latest);
            }
        }
    }

    state.logs.create_diagnostic_report(
        instance_meta,
        game_lines.unwrap_or_default(),
        fallback_latest_log,
        last_error,
    )
}

#[tauri::command]
pub fn logs_open_launcher_folder(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<(), LauncherErrorPayload> {
    let dir = state.logs.logs_dir();
    launcher_security::ensure_dir_within(&state.paths.root, &dir, true)?;

    let dir_str = dir.to_str().ok_or_else(|| {
        LauncherErrorPayload::invalid_input("Path folder log bukan UTF-8 yang valid.")
    })?;

    app.opener()
        .open_path(dir_str, None::<&str>)
        .map_err(|e| LauncherErrorPayload::internal(format!("Gagal membuka folder log: {}", e)))?;

    Ok(())
}
