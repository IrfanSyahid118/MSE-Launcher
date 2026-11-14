use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstallation {
    pub path: String,
    pub version: String,
    pub major_version: u32,
    pub bits: u32,
    pub vendor: Option<String>,
    pub managed: bool,
}
