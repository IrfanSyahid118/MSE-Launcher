use super::settings::JvmPreset;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LoaderType {
    #[default]
    Vanilla,
    Fabric,
    Quilt,
    Neoforge,
    Forge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoaderSpec {
    #[serde(rename = "type")]
    pub loader_type: LoaderType,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum InstanceJavaMode {
    #[default]
    Auto,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceJavaSettings {
    pub mode: InstanceJavaMode,
    pub path: Option<String>,
}

impl Default for InstanceJavaSettings {
    fn default() -> Self {
        Self {
            mode: InstanceJavaMode::Auto,
            path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstanceMemory {
    pub override_global: bool,
    pub min_mb: Option<u32>,
    pub max_mb: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceJvmArgs {
    pub preset: JvmPreset,
    #[serde(default)]
    pub extra: Vec<String>,
}

impl Default for InstanceJvmArgs {
    fn default() -> Self {
        Self {
            preset: JvmPreset::Default,
            extra: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Mod,
    Modpack,
    Shader,
    Resourcepack,
    Datapack,
    World,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentProviderId {
    Modrinth,
    Curseforge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceContentEntry {
    pub provider: ContentProviderId,
    pub project_id: String,
    pub version_id: String,
    #[serde(rename = "type")]
    pub content_type: ContentType,
    pub file_name: String,
    pub sha1: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world_folder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderSpec,
    pub java: InstanceJavaSettings,
    pub memory: InstanceMemory,
    pub jvm_args: InstanceJvmArgs,
    #[serde(default)]
    pub content: Vec<InstanceContentEntry>,
    pub created_at: String,
    pub last_played_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceRequest {
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderSpec,
}
