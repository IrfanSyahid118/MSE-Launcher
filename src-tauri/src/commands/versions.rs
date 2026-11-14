use crate::state::AppState;
use launcher_types::LauncherErrorPayload;
use launcher_version::MinecraftVersionInfo;

#[tauri::command]
pub async fn versions_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<MinecraftVersionInfo>, LauncherErrorPayload> {
    state.versions.list_versions().await
}
