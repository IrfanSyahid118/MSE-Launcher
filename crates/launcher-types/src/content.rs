use super::instance::{ContentProviderId, ContentType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentSort {
    Relevance,
    Downloads,
    Follows,
    Updated,
    Newest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentDependencyKind {
    Required,
    Optional,
    Embedded,
    Incompatible,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentDependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub kind: ContentDependencyKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProjectSummary {
    pub provider: ContentProviderId,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub content_type: ContentType,
    pub downloads: u64,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProject {
    #[serde(flatten)]
    pub summary: ContentProjectSummary,
    pub body: String,
    pub updated_at: String,
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentVersion {
    pub provider: ContentProviderId,
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: u64,
    pub published_at: String,
    pub file_name: String,
    pub download_url: Option<String>,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentFilter {
    pub minecraft_version: Option<String>,
    pub loader: Option<String>,
    pub featured: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub provider: ContentProviderId,
    #[serde(rename = "type")]
    pub content_type: ContentType,
    pub text: String,
    pub page: u32,
    pub page_size: u32,
    pub minecraft_version: Option<String>,
    pub loader: Option<String>,
    pub sort: Option<ContentSort>,
    pub categories: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub items: Vec<ContentProjectSummary>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentPlanEntry {
    pub version: ContentVersion,
    #[serde(rename = "type")]
    pub content_type: ContentType,
    pub downloadable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentInstallPlan {
    pub target: ContentPlanEntry,
    pub dependencies: Vec<ContentPlanEntry>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedWorld {
    pub folder: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentUpdateInfo {
    pub provider: ContentProviderId,
    pub project_id: String,
    pub current_version_id: String,
    pub latest_version_id: String,
    pub latest_version_number: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallModpackRequest {
    pub provider: ContentProviderId,
    pub version_id: String,
    pub instance_name: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
}
