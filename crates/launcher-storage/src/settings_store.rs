use crate::fs_atomic::{read_json_file, write_json_atomic};
use launcher_platform::DataPaths;
use launcher_types::{LauncherErrorPayload, Settings, SettingsPatch};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct SettingsStore {
    paths: DataPaths,
    cache: Arc<RwLock<Settings>>,
    unrecovered_load_error: Arc<RwLock<Option<String>>>,
}

impl SettingsStore {
    pub fn new(paths: DataPaths) -> Self {
        let (initial, load_err) = Self::load_from_disk(&paths);
        Self {
            paths,
            cache: Arc::new(RwLock::new(initial)),
            unrecovered_load_error: Arc::new(RwLock::new(load_err)),
        }
    }

    fn load_from_disk(paths: &DataPaths) -> (Settings, Option<String>) {
        match read_json_file::<Settings>(&paths.settings_file) {
            Ok(Some(s)) => (s, None),
            Ok(None) => {
                let default_settings = Settings::default();
                let _ = write_json_atomic(&paths.settings_file, &default_settings);
                (default_settings, None)
            }
            Err(e) => {
                tracing::error!(
                    "Gagal membaca berkas pengaturan {:?}; memakai default di memori tanpa menimpa disk: {:?}",
                    paths.settings_file,
                    e
                );
                (Settings::default(), Some(e.message))
            }
        }
    }

    pub fn get(&self) -> Settings {
        self.cache.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, patch: SettingsPatch) -> Result<Settings, LauncherErrorPayload> {
        let mut cache_guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
        if let Some(reason) = self
            .unrecovered_load_error
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            if self.paths.settings_file.exists() {
                return Err(LauncherErrorPayload::file_system(format!(
                    "Menolak menimpa settings.json yang gagal dimuat: {}",
                    reason
                )));
            }
        }
        let mut next = cache_guard.clone();

        if let Some(theme) = patch.theme {
            if let Some(mode) = theme.mode {
                next.theme.mode = mode;
            }
            if let Some(reduce_transparency) = theme.reduce_transparency {
                next.theme.reduce_transparency = reduce_transparency;
            }
            if let Some(reduce_motion) = theme.reduce_motion {
                next.theme.reduce_motion = reduce_motion;
            }
        }

        if let Some(mem) = patch.memory {
            let max_mb = mem.default_max_mb.unwrap_or(next.memory.default_max_mb);
            let min_mb = mem.default_min_mb.unwrap_or(next.memory.default_min_mb);
            if min_mb > max_mb {
                return Err(LauncherErrorPayload::invalid_input(
                    "Alokasi memori minimum tidak boleh lebih besar dari alokasi maksimum.",
                ));
            }
            next.memory.default_max_mb = max_mb;
            next.memory.default_min_mb = min_mb;
        }

        if let Some(java) = patch.java {
            if let Some(prefer_managed) = java.prefer_managed {
                next.java.prefer_managed = prefer_managed;
            }
            if let Some(custom_path) = java.custom_path {
                next.java.custom_path = custom_path;
            }
            if let Some(jvm_preset) = java.jvm_preset {
                next.java.jvm_preset = jvm_preset;
            }
            if let Some(custom_jvm_args) = java.custom_jvm_args {
                next.java.custom_jvm_args = custom_jvm_args;
            }
        }

        if let Some(dl) = patch.downloads {
            if let Some(parallelism) = dl.parallelism {
                next.downloads.parallelism = parallelism.clamp(1, 16);
            }
        }

        if let Some(discord) = patch.discord {
            if let Some(enabled) = discord.enabled {
                next.discord.enabled = enabled;
            }
            if let Some(show_idle) = discord.show_idle {
                next.discord.show_idle = show_idle;
            }
            if let Some(show_playing) = discord.show_playing {
                next.discord.show_playing = show_playing;
            }
            if let Some(custom_client_id) = discord.custom_client_id {
                next.discord.custom_client_id = custom_client_id;
            }
        }

        if let Some(lang) = patch.language {
            next.language = lang;
        }

        if let Some(active_account_id) = patch.active_account_id {
            next.active_account_id = active_account_id;
        }

        write_json_atomic(&self.paths.settings_file, &next)?;
        *self
            .unrecovered_load_error
            .write()
            .unwrap_or_else(|e| e.into_inner()) = None;
        *cache_guard = next.clone();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_patch_null_clears_double_option_fields() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(temp.path());
        let store = SettingsStore::new(paths);

        store
            .set(
                serde_json::from_str(
                    r#"{
                        "java": { "customPath": "C:\\Java\\bin\\java.exe", "preferManaged": false },
                        "discord": { "customClientId": "1234567890" },
                        "activeAccountId": "00000000-0000-4000-8000-000000000001"
                    }"#,
                )
                .unwrap(),
            )
            .unwrap();

        let partial_update: SettingsPatch =
            serde_json::from_str(r#"{ "language": "en" }"#).unwrap();
        let after_partial = store.set(partial_update).unwrap();
        assert_eq!(
            after_partial.java.custom_path.as_deref(),
            Some("C:\\Java\\bin\\java.exe")
        );
        assert_eq!(
            after_partial.discord.custom_client_id.as_deref(),
            Some("1234567890")
        );

        let clear_patch: SettingsPatch = serde_json::from_str(
            r#"{
                "java": { "customPath": null, "preferManaged": true },
                "discord": { "customClientId": null },
                "activeAccountId": null
            }"#,
        )
        .unwrap();
        let after_clear = store.set(clear_patch).unwrap();
        assert_eq!(after_clear.java.custom_path, None);
        assert!(after_clear.java.prefer_managed);
        assert_eq!(after_clear.discord.custom_client_id, None);
        assert_eq!(after_clear.active_account_id, None);
    }
}
