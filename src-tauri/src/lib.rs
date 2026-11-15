pub mod commands;
pub mod logs;
pub mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        let msg = launcher_platform::sanitize_log_text(&format!("PANIC: {:?}", info));
        eprintln!("{msg}");
        let log_dir = launcher_platform::resolve_data_root().join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let _ = std::fs::write(log_dir.join("crash.log"), msg);
    }));

    let app_state = AppState::new();
    let _ = app_state.paths.ensure_root_secure();
    let log_writer = app_state.logs.make_tracing_writer();
    let _ = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(log_writer)
        .try_init();

    let res = tauri::Builder::default()
        .setup(|app| {
            let state = app.state::<AppState>();
            state.logs.attach_app_handle(app.handle().clone());
            match state.instances.cleanup_incomplete_modpacks() {
                Ok(0) => {}
                Ok(count) => tracing::info!(
                    "Membersihkan {} instance modpack yang terputus pada sesi sebelumnya",
                    count
                ),
                Err(err) => tracing::warn!(
                    "Gagal memeriksa instance modpack tidak selesai saat startup: {}",
                    err.message
                ),
            }
            let scope = app.asset_protocol_scope();
            let skins_dir = state.paths.root.join("skins");
            if let Err(err) = scope.allow_directory(&skins_dir, true) {
                tracing::warn!(
                    "Gagal mendaftarkan scope assetProtocol untuk skins ({}): {}",
                    skins_dir.display(),
                    err
                );
            }
            if let Err(err) = scope.allow_directory(&state.paths.instances_dir, true) {
                tracing::warn!(
                    "Gagal mendaftarkan scope assetProtocol untuk instances ({}): {}",
                    state.paths.instances_dir.display(),
                    err
                );
            }
            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::system_memory_info,
            commands::settings_get,
            commands::settings_set,
            commands::instances_list,
            commands::instances_create,
            commands::instances_update,
            commands::instances_remove,
            commands::instances_open_folder,
            commands::downloads_cleanup,
            commands::accounts_list,
            commands::accounts_set_active,
            commands::accounts_update_skin,
            commands::accounts_sync_minecraft,
            commands::accounts_set_active_cape,
            commands::accounts_install_skin_resource_pack,
            commands::accounts_export_skin_resource_pack,
            commands::auth_create_offline,
            commands::auth_begin_microsoft,
            commands::auth_complete_microsoft,
            commands::auth_sign_out,
            commands::shell_open_external,
            commands::versions_list,
            commands::java_detect,
            commands::java_validate,
            commands::java_ensure,
            commands::launch_start,
            commands::launch_kill,
            commands::tasks_cancel,
            commands::loaders_games,
            commands::loaders_versions,
            commands::servers_list,
            commands::servers_add,
            commands::servers_remove,
            commands::servers_edit,
            commands::servers_ping,
            commands::content_search,
            commands::content_project,
            commands::content_versions,
            commands::content_plan,
            commands::content_install,
            commands::content_remove,
            commands::content_toggle,
            commands::content_worlds,
            commands::content_check_updates,
            commands::content_install_modpack,
            commands::integrations_status,
            commands::integrations_set_curse_forge_key,
            commands::integrations_set_azure_client_id,
            commands::logs_launcher_history,
            commands::logs_append_client,
            commands::logs_clear_launcher,
            commands::logs_create_diagnostic,
            commands::logs_open_launcher_folder,
            commands::updater_check,
            commands::updater_install,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = res {
        let safe_err = launcher_platform::sanitize_log_text(&format!("{e:?}"));
        eprintln!("error while running tauri application: {safe_err}");
        let log_dir = launcher_platform::resolve_data_root().join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let _ = std::fs::write(log_dir.join("launch_error.log"), safe_err);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_n34_asset_protocol_enabled_and_scoped() {
        let cargo_toml = include_str!("../Cargo.toml");
        assert!(
            cargo_toml.contains("protocol-asset"),
            "tauri dependency must enable protocol-asset feature"
        );

        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let asset_proto = &conf["app"]["security"]["assetProtocol"];
        assert_eq!(asset_proto["enable"], serde_json::json!(true));
        let scope = asset_proto["scope"].as_array().expect("scope array");
        assert!(!scope.is_empty());
        assert!(!scope.iter().any(|v| v.as_str() == Some("**")));
    }
}
