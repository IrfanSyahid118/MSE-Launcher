use crate::state::AppState;
use launcher_types::{IntegrationsStatus, LauncherErrorPayload};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCurseForgeKeyRequest {
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAzureClientIdRequest {
    pub client_id: Option<String>,
}

#[tauri::command]
pub fn integrations_status(
    state: State<'_, AppState>,
) -> Result<IntegrationsStatus, LauncherErrorPayload> {
    Ok(state.content.integrations_status())
}

#[tauri::command]
pub fn integrations_set_curse_forge_key(
    state: State<'_, AppState>,
    req: SetCurseForgeKeyRequest,
) -> Result<IntegrationsStatus, LauncherErrorPayload> {
    state.content.set_curseforge_key(req.api_key.as_deref())
}

#[tauri::command]
pub fn integrations_set_azure_client_id(
    state: State<'_, AppState>,
    req: SetAzureClientIdRequest,
) -> Result<IntegrationsStatus, LauncherErrorPayload> {
    state.content.set_azure_client_id(req.client_id.as_deref())
}
