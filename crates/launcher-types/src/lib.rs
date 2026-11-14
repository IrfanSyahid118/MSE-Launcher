pub mod account;
pub mod content;
pub mod instance;
pub mod java;
pub mod server;
pub mod settings;

use serde::{Deserialize, Serialize};

pub use account::*;
pub use content::*;
pub use instance::*;
pub use java::*;
pub use server::*;
pub use settings::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NotFound,
    InvalidInput,
    AlreadyExists,
    Network,
    RateLimited,
    AuthFailed,
    AuthExpired,
    ProviderNotConfigured,
    DistributionBlocked,
    FileSystem,
    LaunchFailed,
    DiskFull,
    JavaMissing,
    HashMismatch,
    Cancelled,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherErrorPayload {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub configured: bool,
    pub source: Option<String>,
    pub stored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationsStatus {
    pub curseforge: IntegrationStatus,
    #[serde(default)]
    pub azure: Option<IntegrationStatus>,
}

impl std::fmt::Display for LauncherErrorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for LauncherErrorPayload {}

impl LauncherErrorPayload {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: None,
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn auth_failed(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::AuthFailed, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Network, message)
    }

    pub fn file_system(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::FileSystem, message)
    }

    pub fn provider_not_configured(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ProviderNotConfigured, message)
    }

    pub fn distribution_blocked(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::DistributionBlocked, message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_mb: u64,
    pub free_mb: u64,
    pub used_mb: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallMode {
    Installer,
    Portable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub install_mode: InstallMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
}
