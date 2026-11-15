use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    System,
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Id,
    En,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct ThemeSettings {
    pub mode: ThemeMode,
    pub reduce_transparency: bool,
    pub reduce_motion: bool,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            reduce_transparency: false,
            reduce_motion: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct MemorySettings {
    pub default_max_mb: u32,
    pub default_min_mb: u32,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            default_max_mb: 4096,
            default_min_mb: 2048,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum JvmPreset {
    Default,
    #[default]
    Performance,
    LowMemory,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct JavaSettings {
    pub prefer_managed: bool,
    pub custom_path: Option<String>,
    pub jvm_preset: JvmPreset,
    pub custom_jvm_args: String,
}

impl Default for JavaSettings {
    fn default() -> Self {
        Self {
            prefer_managed: true,
            custom_path: None,
            jvm_preset: JvmPreset::Performance,
            custom_jvm_args: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct DownloadSettings {
    pub parallelism: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self { parallelism: 6 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct DiscordSettings {
    pub enabled: bool,
    pub show_idle: bool,
    pub show_playing: bool,
    pub custom_client_id: Option<String>,
}

impl Default for DiscordSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            show_idle: true,
            show_playing: true,
            custom_client_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u32,
    pub theme: ThemeSettings,
    pub memory: MemorySettings,
    pub java: JavaSettings,
    pub downloads: DownloadSettings,
    pub discord: DiscordSettings,
    pub language: Language,
    pub active_account_id: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: ThemeSettings::default(),
            memory: MemorySettings::default(),
            java: JavaSettings::default(),
            downloads: DownloadSettings::default(),
            discord: DiscordSettings::default(),
            language: Language::default(),
            active_account_id: None,
        }
    }
}

// Distinguish omitted JSON fields (None) from explicit null (Some(None)) so patches can clear values.
fn deserialize_double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettingsPatch {
    pub mode: Option<ThemeMode>,
    pub reduce_transparency: Option<bool>,
    pub reduce_motion: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySettingsPatch {
    pub default_max_mb: Option<u32>,
    pub default_min_mb: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaSettingsPatch {
    pub prefer_managed: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub custom_path: Option<Option<String>>,
    pub jvm_preset: Option<JvmPreset>,
    pub custom_jvm_args: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSettingsPatch {
    pub parallelism: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordSettingsPatch {
    pub enabled: Option<bool>,
    pub show_idle: Option<bool>,
    pub show_playing: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub custom_client_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub theme: Option<ThemeSettingsPatch>,
    pub memory: Option<MemorySettingsPatch>,
    pub java: Option<JavaSettingsPatch>,
    pub downloads: Option<DownloadSettingsPatch>,
    pub discord: Option<DiscordSettingsPatch>,
    pub language: Option<Language>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub active_account_id: Option<Option<String>>,
}
