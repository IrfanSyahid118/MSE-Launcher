use crate::state::AppState;
use launcher_loader::{LoaderGameSupport, LoaderVersionInfo};
use launcher_types::{LauncherErrorPayload, LoaderType};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadersVersionsRequest {
    #[serde(alias = "type")]
    pub loader_type: LoaderType,
    pub minecraft_version: String,
}

#[tauri::command]
pub async fn loaders_games(
    state: State<'_, AppState>,
) -> Result<LoaderGameSupport, LauncherErrorPayload> {
    state.loaders.get_game_support().await
}

#[tauri::command]
pub async fn loaders_versions(
    state: State<'_, AppState>,
    req: LoadersVersionsRequest,
) -> Result<Vec<LoaderVersionInfo>, LauncherErrorPayload> {
    state
        .loaders
        .list_versions(req.loader_type, &req.minecraft_version)
        .await
}
