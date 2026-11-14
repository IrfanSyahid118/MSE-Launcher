use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Microsoft,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountTokens {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountCape {
    pub id: String,
    pub state: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub username: String,
    pub uuid: String,
    #[serde(rename = "type")]
    pub account_type: AccountType,
    pub created_at: String,
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<AccountTokens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin_variant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cape_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_cape_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capes: Vec<AccountCape>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountSkinRequest {
    pub account_id: String,
    pub skin_data: String,
    pub skin_variant: Option<String>,
    pub cape_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMinecraftProfileRequest {
    pub account_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveCapeRequest {
    pub account_id: String,
    #[serde(default)]
    pub cape_id: Option<String>,
    #[serde(default)]
    pub custom_cape_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallSkinResourcePackRequest {
    pub account_id: String,
    pub instance_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallSkinResourcePackResult {
    pub updated_instances: u32,
    pub pack_file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountsFile {
    pub schema_version: u32,
    #[serde(default)]
    pub accounts: Vec<Account>,
}

impl Default for AccountsFile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            accounts: Vec::new(),
        }
    }
}
