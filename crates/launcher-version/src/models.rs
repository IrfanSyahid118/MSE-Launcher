use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MinecraftVersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersionInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: MinecraftVersionType,
    pub release_time: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangLatest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MojangVersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    pub time: String,
    pub release_time: String,
    pub sha1: String,
    pub compliance_level: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangVersionList {
    pub latest: MojangLatest,
    pub versions: Vec<MojangVersionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangDownloadArtifact {
    pub path: Option<String>,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangDownloads {
    pub client: Option<MojangDownloadArtifact>,
    pub server: Option<MojangDownloadArtifact>,
    pub client_mappings: Option<MojangDownloadArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MojangAssetIndexInfo {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangLibraryDownloads {
    pub artifact: Option<MojangDownloadArtifact>,
    pub classifiers: Option<std::collections::HashMap<String, MojangDownloadArtifact>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MojangLibrary {
    pub name: String,
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub downloads: Option<MojangLibraryDownloads>,
    pub rules: Option<Vec<crate::rules::MojangRule>>,
    pub natives: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MojangJavaVersionInfo {
    pub component: Option<String>,
    pub major_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MojangVersionDetail {
    pub id: String,
    pub inherits_from: Option<String>,
    pub main_class: Option<String>,
    pub assets: Option<String>,
    pub asset_index: Option<MojangAssetIndexInfo>,
    pub downloads: Option<MojangDownloads>,
    pub java_version: Option<MojangJavaVersionInfo>,
    #[serde(default)]
    pub libraries: Vec<MojangLibrary>,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<serde_json::Value>,
}
