use crate::state::AppState;
use launcher_types::{LauncherErrorPayload, Settings, SettingsPatch};

#[tauri::command]
pub fn settings_get(state: tauri::State<AppState>) -> Result<Settings, LauncherErrorPayload> {
    Ok(state.settings.get())
}

#[tauri::command]
pub fn settings_set(
    state: tauri::State<AppState>,
    patch: SettingsPatch,
) -> Result<Settings, LauncherErrorPayload> {
    let new_settings = state.settings.set(patch)?;
    apply_runtime_settings(&new_settings, &state.downloader, &state.discord);
    Ok(new_settings)
}

fn apply_runtime_settings(
    new_settings: &Settings,
    downloader: &launcher_download::DownloadManager,
    discord: &launcher_discord::DiscordRpcService,
) {
    downloader.set_parallelism(new_settings.downloads.parallelism as usize);
    discord.update_settings(new_settings.discord.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_types::DownloadSettingsPatch;

    #[test]
    fn test_n23_settings_set_updates_downloader_parallelism_without_restart() {
        let root = std::env::temp_dir().join(format!("mcl-n23-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = launcher_platform::DataPaths::new(&root);
        let store = launcher_storage::SettingsStore::new(paths);
        let downloader =
            launcher_download::DownloadManager::new(store.get().downloads.parallelism as usize);
        let cloned_service_downloader = downloader.clone();
        let mut discord_cfg = store.get().discord;
        discord_cfg.enabled = false;
        let discord = launcher_discord::DiscordRpcService::new(discord_cfg);

        let new_settings = store
            .set(SettingsPatch {
                downloads: Some(DownloadSettingsPatch {
                    parallelism: Some(12),
                }),
                ..Default::default()
            })
            .unwrap();
        apply_runtime_settings(&new_settings, &downloader, &discord);

        let updated_parallelism = cloned_service_downloader.parallelism();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            updated_parallelism, 12,
            "DownloadManager parallelism must update immediately when settings change"
        );
    }
}
