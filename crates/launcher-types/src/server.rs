use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerResourcePackPolicy {
    Prompt,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceServerEntry {
    pub name: String,
    pub ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_textures: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_pack_policy: Option<ServerResourcePackPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_from_home: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerPingStatus {
    pub ip: String,
    pub online: bool,
    pub latency_ms: Option<u32>,
    pub players_online: Option<u32>,
    pub players_max: Option<u32>,
    pub motd: Option<String>,
    pub version_name: Option<String>,
    pub favicon: Option<String>,
    pub checked_at: String,
}
