use crate::fs_atomic::{read_json_file, write_json_atomic};
use launcher_platform::DataPaths;
use launcher_types::{LauncherErrorPayload, Settings, SettingsPatch};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct SettingsStore {
    paths: DataPaths,
    cache: Arc<RwLock<Settings>>,
}

impl SettingsStore {
    pub fn new(paths: DataPaths) -> Self {
        let initial = Self::load_from_disk(&paths);
        Self {
            paths,
            cache: Arc::new(RwLock::new(initial)),
        }
    }

    fn load_from_disk(paths: &DataPaths) -> Settings {
        match read_json_file::<Settings>(&paths.settings_file) {
            Ok(Some(s)) => s,
            _ => {
                let default_settings = Settings::default();
                let _ = write_json_atomic(&paths.settings_file, &default_settings);
                default_settings
            }
        }
    }

    pub fn get(&self) -> Settings {
        self.cache.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, patch: SettingsPatch) -> Result<Settings, LauncherErrorPayload> {
        let mut current = self.cache.write().unwrap_or_else(|e| e.into_inner());

        if let Some(theme) = patch.theme {
            if let Some(mode) = theme.mode {
                current.theme.mode = mode;
            }
            if let Some(reduce_transparency) = theme.reduce_transparency {
                current.theme.reduce_transparency = reduce_transparency;
            }
            if let Some(reduce_motion) = theme.reduce_motion {
                current.theme.reduce_motion = reduce_motion;
            }
        }

        if let Some(mem) = patch.memory {
            let max_mb = mem.default_max_mb.unwrap_or(current.memory.default_max_mb);
            let min_mb = mem.default_min_mb.unwrap_or(current.memory.default_min_mb);
            if min_mb > max_mb {
                return Err(LauncherErrorPayload::invalid_input(
                    "Alokasi memori minimum tidak boleh lebih besar dari alokasi maksimum.",
                ));
            }
            current.memory.default_max_mb = max_mb;
            current.memory.default_min_mb = min_mb;
        }

        if let Some(java) = patch.java {
            if let Some(prefer_managed) = java.prefer_managed {
                current.java.prefer_managed = prefer_managed;
            }
            if let Some(custom_path) = java.custom_path {
                current.java.custom_path = custom_path;
            }
            if let Some(jvm_preset) = java.jvm_preset {
                current.java.jvm_preset = jvm_preset;
            }
            if let Some(custom_jvm_args) = java.custom_jvm_args {
                current.java.custom_jvm_args = custom_jvm_args;
            }
        }

        if let Some(dl) = patch.downloads {
            if let Some(parallelism) = dl.parallelism {
                current.downloads.parallelism = parallelism.clamp(1, 16);
            }
        }

        if let Some(discord) = patch.discord {
            if let Some(enabled) = discord.enabled {
                current.discord.enabled = enabled;
            }
            if let Some(show_idle) = discord.show_idle {
                current.discord.show_idle = show_idle;
            }
            if let Some(show_playing) = discord.show_playing {
                current.discord.show_playing = show_playing;
            }
            if let Some(custom_client_id) = discord.custom_client_id {
                current.discord.custom_client_id = custom_client_id;
            }
        }

        if let Some(lang) = patch.language {
            current.language = lang;
        }

        if let Some(active_account_id) = patch.active_account_id {
            current.active_account_id = active_account_id;
        }

        write_json_atomic(&self.paths.settings_file, &*current)?;
        Ok(current.clone())
    }
}
