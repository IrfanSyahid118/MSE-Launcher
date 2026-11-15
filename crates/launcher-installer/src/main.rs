#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod payload;
mod windows_ops;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallerState {
    mode: String,
    app_version: String,
    has_payload: bool,
    default_install_dir: String,
    default_portable_dir: String,
    uninstall_dir: Option<String>,
    user_data_dir: Option<String>,
    default_lang: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallOptions {
    install_mode: String,
    target_dir: String,
    desktop_shortcut: bool,
    start_menu_shortcut: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    percent: u8,
    stage: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    installed_exe_path: Option<String>,
}

fn is_uninstall_invocation() -> bool {
    if std::env::args().any(|arg| arg.eq_ignore_ascii_case("--uninstall")) {
        return true;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_ascii_lowercase())
        })
        .is_some_and(|name| name.starts_with("unins"))
}

fn emit_progress(app: &tauri::AppHandle, percent: u8, stage: &str) {
    let _ = app.emit(
        "installer:progress",
        ProgressEvent {
            percent,
            stage: stage.to_string(),
        },
    );
}

#[tauri::command]
fn installer_get_state() -> InstallerState {
    let uninstall_mode = is_uninstall_invocation();
    let current_exe = std::env::current_exe().ok();

    let (has_payload, app_version) = current_exe
        .as_deref()
        .and_then(|exe| payload::read_payload_manifest(exe).ok())
        .map(|(_, manifest)| (true, manifest.version))
        .unwrap_or_else(|| (false, env!("CARGO_PKG_VERSION").to_string()));

    let uninstall_dir = if uninstall_mode {
        windows_ops::read_registered_install_location()
            .or_else(|| {
                current_exe
                    .as_deref()
                    .and_then(|p| p.parent().map(PathBuf::from))
            })
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };

    InstallerState {
        mode: if uninstall_mode {
            "uninstall".to_string()
        } else {
            "install".to_string()
        },
        app_version,
        has_payload,
        default_install_dir: windows_ops::default_install_dir()
            .to_string_lossy()
            .to_string(),
        default_portable_dir: windows_ops::default_portable_dir()
            .to_string_lossy()
            .to_string(),
        uninstall_dir,
        user_data_dir: windows_ops::user_data_dir().map(|p| p.to_string_lossy().to_string()),
        default_lang: windows_ops::detect_system_language(),
    }
}

#[tauri::command]
async fn installer_pick_folder(
    initial_path: Option<String>,
    title: String,
) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        windows_ops::pick_folder_dialog(initial_path.as_deref(), &title)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn installer_run_install(
    app: tauri::AppHandle,
    options: InstallOptions,
) -> Result<DoneEvent, String> {
    let target_trimmed = options.target_dir.trim().to_string();
    if target_trimmed.is_empty() {
        return Err("Target directory cannot be empty.".to_string());
    }

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;

    let result = tokio::task::spawn_blocking(move || -> Result<DoneEvent, String> {
        let (payload_offset, manifest) = payload::read_payload_manifest(&current_exe)
            .map_err(|e| format!("Payload error: {e}"))?;

        let target_dir = PathBuf::from(&target_trimmed);
        let is_portable = options.install_mode == "portable";

        emit_progress(&app, 12, "closing");
        windows_ops::close_running_launcher();

        emit_progress(&app, 32, "extracting");
        std::fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create target directory: {e}"))?;

        let exe_dest = target_dir.join(&manifest.app_exe_name);
        let extracted_bytes = payload::extract_payload_app(
            &current_exe,
            payload_offset,
            &manifest.app_exe_name,
            &exe_dest,
        )
        .map_err(|e| format!("Failed to extract application: {e}"))?;

        emit_progress(
            &app,
            65,
            if is_portable {
                "portable"
            } else {
                "uninstaller"
            },
        );
        if is_portable {
            let marker = target_dir.join("portable.txt");
            std::fs::write(&marker, b"portable\n")
                .map_err(|e| format!("Failed to write portable marker: {e}"))?;
        } else {
            let uninstaller_dest = target_dir.join("uninstall.exe");
            payload::copy_stub_without_payload(&current_exe, payload_offset, &uninstaller_dest)
                .map_err(|e| format!("Failed to write uninstaller: {e}"))?;

            emit_progress(&app, 82, "shortcuts");
            if options.desktop_shortcut {
                if let Some(desktop_lnk) = windows_ops::desktop_shortcut_path() {
                    windows_ops::create_shortcut(&exe_dest, &desktop_lnk, "MSE Launcher")?;
                }
            }
            if options.start_menu_shortcut {
                if let Some(start_lnk) = windows_ops::start_menu_shortcut_path() {
                    windows_ops::create_shortcut(&exe_dest, &start_lnk, "MSE Launcher")?;
                }
            }

            emit_progress(&app, 93, "registry");
            let size_kb = (extracted_bytes / 1024).max(1);
            windows_ops::register_uninstall_entry(
                &target_dir,
                &exe_dest,
                &uninstaller_dest,
                &manifest.version,
                size_kb,
            )?;
        }

        if is_portable && options.desktop_shortcut {
            emit_progress(&app, 85, "shortcuts");
            if let Some(desktop_lnk) = windows_ops::desktop_shortcut_path() {
                windows_ops::create_shortcut(&exe_dest, &desktop_lnk, "MSE Launcher Portable")?;
            }
        }

        emit_progress(&app, 100, "done");
        Ok(DoneEvent {
            installed_exe_path: Some(exe_dest.to_string_lossy().to_string()),
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(result)
}

#[tauri::command]
async fn installer_run_uninstall(
    app: tauri::AppHandle,
    remove_user_data: bool,
) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let install_dir = windows_ops::read_registered_install_location()
        .or_else(|| current_exe.parent().map(PathBuf::from))
        .ok_or_else(|| "Unable to determine install directory.".to_string())?;

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        emit_progress(&app, 15, "closing");
        windows_ops::close_running_launcher();

        emit_progress(&app, 40, "shortcuts");
        if let Some(desktop_lnk) = windows_ops::desktop_shortcut_path() {
            let _ = std::fs::remove_file(desktop_lnk);
        }
        if let Some(start_lnk) = windows_ops::start_menu_shortcut_path() {
            let _ = std::fs::remove_file(start_lnk);
        }

        emit_progress(&app, 60, "registry");
        windows_ops::unregister_uninstall_entry();

        emit_progress(&app, 80, "removing");
        for file_name in [
            "MSE Launcher.exe",
            "minecraft-launcher-desktop.exe",
            "portable.txt",
        ] {
            let candidate = install_dir.join(file_name);
            if candidate.exists() {
                let _ = std::fs::remove_file(&candidate);
            }
        }

        if remove_user_data {
            if let Some(data_dir) = windows_ops::user_data_dir() {
                if data_dir.exists() {
                    let _ = std::fs::remove_dir_all(&data_dir);
                }
            }
        }

        emit_progress(&app, 100, "done");
        windows_ops::schedule_self_cleanup(&current_exe, &install_dir);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn installer_launch_and_exit(
    app: tauri::AppHandle,
    exe_path: Option<String>,
) -> Result<(), String> {
    if let Some(path) = exe_path.filter(|p| !p.trim().is_empty()) {
        windows_ops::launch_executable(&PathBuf::from(path))?;
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn installer_window_minimize(window: tauri::WebviewWindow) {
    let _ = window.minimize();
}

#[tauri::command]
fn installer_window_close(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            installer_get_state,
            installer_pick_folder,
            installer_run_install,
            installer_run_uninstall,
            installer_launch_and_exit,
            installer_window_minimize,
            installer_window_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MSE Launcher Setup");
}
