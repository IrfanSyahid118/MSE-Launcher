use crate::state::AppState;
use launcher_types::{
    InstanceServerEntry, LauncherErrorPayload, ServerPingStatus, ServerResourcePackPolicy,
};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServersAddRequest {
    pub instance_id: String,
    pub name: String,
    pub ip: String,
    pub resource_pack_policy: Option<ServerResourcePackPolicy>,
    pub hide_from_home: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServersRemoveRequest {
    pub instance_id: String,
    pub ip: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServersEditRequest {
    pub instance_id: String,
    pub old_ip: String,
    pub name: String,
    pub new_ip: String,
    pub resource_pack_policy: Option<ServerResourcePackPolicy>,
    pub hide_from_home: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServersPingRequest {
    pub ip: String,
    pub instance_id: Option<String>,
}

#[tauri::command]
pub fn servers_list(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<InstanceServerEntry>, LauncherErrorPayload> {
    state.servers.list(&instance_id)
}

#[tauri::command]
pub fn servers_add(
    state: State<'_, AppState>,
    req: ServersAddRequest,
) -> Result<Vec<InstanceServerEntry>, LauncherErrorPayload> {
    let entry = InstanceServerEntry {
        name: req.name,
        ip: req.ip,
        icon: None,
        accept_textures: None,
        resource_pack_policy: req.resource_pack_policy,
        hide_from_home: req.hide_from_home,
    };
    state.servers.add(&req.instance_id, entry)
}

#[tauri::command]
pub fn servers_remove(
    state: State<'_, AppState>,
    req: ServersRemoveRequest,
) -> Result<Vec<InstanceServerEntry>, LauncherErrorPayload> {
    state.servers.remove(&req.instance_id, &req.ip)
}

#[tauri::command]
pub fn servers_edit(
    state: State<'_, AppState>,
    req: ServersEditRequest,
) -> Result<Vec<InstanceServerEntry>, LauncherErrorPayload> {
    let updated = InstanceServerEntry {
        name: req.name,
        ip: req.new_ip,
        icon: None,
        accept_textures: None,
        resource_pack_policy: req.resource_pack_policy,
        hide_from_home: req.hide_from_home,
    };
    state.servers.edit(&req.instance_id, &req.old_ip, updated)
}

#[tauri::command]
pub async fn servers_ping(
    state: State<'_, AppState>,
    req: ServersPingRequest,
) -> Result<ServerPingStatus, LauncherErrorPayload> {
    state
        .servers
        .ping(&req.ip, req.instance_id.as_deref())
        .await
}
