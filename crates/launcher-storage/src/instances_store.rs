use crate::fs_atomic::{read_json_file, write_json_atomic};
use chrono::Utc;
use launcher_platform::DataPaths;
use launcher_security::{slugify_instance_name, validate_instance_id};
use launcher_types::{
    CreateInstanceRequest, Instance, InstanceJavaSettings, InstanceJvmArgs, InstanceMemory,
    LauncherErrorPayload,
};
use std::fs;
use std::sync::{Arc, Mutex};

const INSTANCE_MINECRAFT_DIRS: &[&str] = &[
    "mods",
    "saves",
    "shaderpacks",
    "resourcepacks",
    "config",
    "logs",
];

pub const MODPACK_INSTALLING_MARKER: &str = ".modpack-installing";
const MAX_INSTANCE_ICON_DATA_URL_LEN: usize = 512 * 1024;
const MAX_INSTANCE_ICON_HTTP_URL_LEN: usize = 2048;

fn sanitize_instance_icon_url(raw: Option<String>) -> Result<Option<String>, LauncherErrorPayload> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if let Some(rest) = trimmed.strip_prefix("data:") {
        let lower = trimmed.to_ascii_lowercase();
        let allowed_prefix = lower.starts_with("data:image/png;base64,")
            || lower.starts_with("data:image/jpeg;base64,")
            || lower.starts_with("data:image/webp;base64,");
        if !allowed_prefix
            || trimmed.len() > MAX_INSTANCE_ICON_DATA_URL_LEN
            || !rest.contains(";base64,")
            || trimmed
                .split_once(',')
                .is_none_or(|(_, b64)| b64.is_empty())
        {
            return Err(LauncherErrorPayload::invalid_input(
                "Format data URI ikon instance tidak valid (hanya PNG/JPEG/WebP base64 maksimal 512 KB).",
            ));
        }
        return Ok(Some(trimmed.to_string()));
    }

    let parsed = url::Url::parse(trimmed)
        .map_err(|_| LauncherErrorPayload::invalid_input("URL ikon instance tidak valid."))?;
    let scheme_ok = match parsed.scheme() {
        "https" => true,
        #[cfg(any(test, debug_assertions))]
        "http" => parsed
            .host_str()
            .is_some_and(|h| matches!(h.to_ascii_lowercase().as_str(), "localhost" | "127.0.0.1")),
        _ => false,
    };
    if !scheme_ok || trimmed.len() > MAX_INSTANCE_ICON_HTTP_URL_LEN {
        return Err(LauncherErrorPayload::invalid_input(
            "URL ikon instance harus menggunakan HTTPS yang valid.",
        ));
    }

    Ok(Some(trimmed.to_string()))
}

#[derive(Clone)]
pub struct InstancesStore {
    paths: DataPaths,
    write_lock: Arc<Mutex<()>>,
}

impl InstancesStore {
    pub fn new(paths: DataPaths) -> Self {
        Self {
            paths,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn list(&self) -> Result<Vec<Instance>, LauncherErrorPayload> {
        let dir = &self.paths.instances_dir;
        if !dir.exists() {
            launcher_security::ensure_secure_dir(dir, true)?;
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(dir).map_err(|e| {
            LauncherErrorPayload::internal(format!("Gagal membaca folder instances: {}", e))
        })?;

        let mut instances = Vec::new();

        for entry in entries.flatten() {
            // file_type() inspects the entry itself without traversing junctions or symlinks.
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if !ft.is_dir() {
                continue;
            }
            let Some(dir_name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if validate_instance_id(&dir_name).is_err() {
                continue;
            }
            let entry_path = entry.path();
            let instance_file = entry_path.join("instance.json");
            if fs::symlink_metadata(&instance_file).is_ok()
                && launcher_security::ensure_descendants_no_symlink(dir, &instance_file).is_ok()
            {
                match read_json_file::<Instance>(&instance_file) {
                    Ok(Some(instance)) => instances.push(instance),
                    _ => {
                        tracing::warn!("Instance di {:?} korup atau tidak valid", entry_path);
                    }
                }
            }
        }

        instances.sort_by(|a, b| {
            b.last_played_at
                .as_ref()
                .unwrap_or(&b.created_at)
                .cmp(a.last_played_at.as_ref().unwrap_or(&a.created_at))
        });

        Ok(instances)
    }

    pub fn get(&self, id: &str) -> Result<Instance, LauncherErrorPayload> {
        validate_instance_id(id)?;
        let instance_dir = launcher_security::safe_subpath(&self.paths.instances_dir, id)?;
        let instance_file = launcher_security::safe_subpath(&instance_dir, "instance.json")?;
        launcher_security::ensure_descendants_no_symlink(
            &self.paths.instances_dir,
            &instance_file,
        )?;
        match read_json_file::<Instance>(&instance_file)? {
            Some(instance) => Ok(instance),
            None => Err(LauncherErrorPayload::not_found(format!(
                "Instance dengan ID '{}' tidak ditemukan.",
                id
            ))),
        }
    }

    fn bounded_suffixed_id(base_id: &str, counter: usize) -> String {
        let suffix = format!("-{}", counter);
        let max_prefix = 128usize.saturating_sub(suffix.len());
        let mut prefix = if base_id.len() > max_prefix {
            base_id[..max_prefix].trim_end_matches('-')
        } else {
            base_id
        };
        if prefix.is_empty() {
            prefix = "instance";
        }
        format!("{}{}", prefix, suffix)
    }

    fn create_locked(&self, req: CreateInstanceRequest) -> Result<Instance, LauncherErrorPayload> {
        let trimmed_name = req.name.trim();
        if trimmed_name.is_empty() {
            return Err(LauncherErrorPayload::invalid_input(
                "Nama instance tidak boleh kosong.",
            ));
        }

        launcher_security::ensure_secure_dir(&self.paths.instances_dir, true)?;

        let base_id = slugify_instance_name(trimmed_name);
        let mut counter = 0usize;
        let (id, instance_dir) = loop {
            let candidate = if counter == 0 {
                base_id.clone()
            } else {
                Self::bounded_suffixed_id(&base_id, counter)
            };
            validate_instance_id(&candidate)?;
            let dir = launcher_security::safe_subpath(&self.paths.instances_dir, &candidate)?;
            match fs::create_dir(&dir) {
                Ok(()) => break (candidate, dir),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    counter = counter.saturating_add(1);
                    if counter > 10_000 {
                        return Err(LauncherErrorPayload::internal(
                            "Tidak dapat mengalokasikan ID instance unik.",
                        ));
                    }
                }
                Err(e) => {
                    return Err(LauncherErrorPayload::internal(format!(
                        "Gagal membuat folder instance {:?}: {}",
                        dir, e
                    )));
                }
            }
        };

        let init_res: Result<Instance, LauncherErrorPayload> = (|| {
            let minecraft_dir = instance_dir.join("minecraft");
            for sub in INSTANCE_MINECRAFT_DIRS {
                let sub_path = minecraft_dir.join(sub);
                fs::create_dir_all(&sub_path).map_err(|e| {
                    LauncherErrorPayload::internal(format!(
                        "Gagal membuat subdirektori {:?}: {}",
                        sub_path, e
                    ))
                })?;
            }

            let now = Utc::now().to_rfc3339();
            let instance = Instance {
                schema_version: 1,
                id: id.clone(),
                name: trimmed_name.to_string(),
                minecraft_version: req.minecraft_version,
                loader: req.loader,
                java: InstanceJavaSettings::default(),
                memory: InstanceMemory::default(),
                jvm_args: InstanceJvmArgs::default(),
                content: Vec::new(),
                created_at: now,
                last_played_at: None,
                icon_url: None,
            };

            let instance_file = instance_dir.join("instance.json");
            write_json_atomic(&instance_file, &instance)?;
            Ok(instance)
        })();

        if init_res.is_err() {
            let _ = launcher_security::safe_remove_dir_all_within(
                &self.paths.instances_dir,
                &instance_dir,
            );
        }

        init_res
    }

    pub fn create(&self, req: CreateInstanceRequest) -> Result<Instance, LauncherErrorPayload> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.create_locked(req)
    }

    pub fn update(&self, mut instance: Instance) -> Result<(), LauncherErrorPayload> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        validate_instance_id(&instance.id)?;
        instance.icon_url = sanitize_instance_icon_url(instance.icon_url)?;
        let instance_dir =
            launcher_security::safe_subpath(&self.paths.instances_dir, &instance.id)?;
        if !instance_dir.exists() {
            return Err(LauncherErrorPayload::not_found(format!(
                "Instance '{}' tidak ditemukan.",
                instance.id
            )));
        }

        let instance_file = launcher_security::safe_subpath(&instance_dir, "instance.json")?;
        launcher_security::ensure_descendants_no_symlink(
            &self.paths.instances_dir,
            &instance_file,
        )?;
        write_json_atomic(&instance_file, &instance)?;
        Ok(())
    }

    pub fn cleanup_incomplete_modpacks(&self) -> Result<usize, LauncherErrorPayload> {
        let dir = &self.paths.instances_dir;
        if !dir.exists() {
            return Ok(0);
        }

        let entries = fs::read_dir(dir).map_err(|e| {
            LauncherErrorPayload::internal(format!("Gagal membaca folder instances: {}", e))
        })?;

        let mut cleaned = 0;
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let path = entry.path();
            let marker = path.join(MODPACK_INSTALLING_MARKER);
            if fs::symlink_metadata(&marker).is_ok() {
                if let Err(err) = launcher_security::safe_remove_dir_all_within(dir, &path) {
                    tracing::warn!(
                        "Gagal membersihkan instance modpack tidak selesai {:?}: {}",
                        path,
                        err.message
                    );
                } else {
                    cleaned += 1;
                }
            }
        }

        Ok(cleaned)
    }

    pub fn remove(&self, id: &str) -> Result<(), LauncherErrorPayload> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        validate_instance_id(id)?;
        let instance_dir = launcher_security::safe_subpath(&self.paths.instances_dir, id)?;
        if fs::symlink_metadata(&instance_dir).is_ok() {
            launcher_security::safe_remove_dir_all_within(
                &self.paths.instances_dir,
                &instance_dir,
            )?;
        }
        Ok(())
    }

    pub fn ensure_default(&self, version_id: &str) -> Result<Instance, LauncherErrorPayload> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(existing) = self.get("default") {
            return Ok(existing);
        }

        // Remove stale or corrupt empty directory at instances/default so default-1 duplicates aren't created.
        let default_dir = launcher_security::safe_subpath(&self.paths.instances_dir, "default")?;
        if fs::symlink_metadata(&default_dir).is_ok() {
            launcher_security::safe_remove_dir_all_within(&self.paths.instances_dir, &default_dir)?;
        }

        self.create_locked(CreateInstanceRequest {
            name: "Default".to_string(),
            minecraft_version: version_id.to_string(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Vanilla,
                version: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_real_instance_json() {
        let json_str = r#"{
          "schemaVersion": 1,
          "id": "0b4992bd-451f-447f-a028-d523b1284297",
          "name": "Fabulously Optimized",
          "minecraftVersion": "26.2",
          "loader": {
            "type": "fabric",
            "version": "0.20.1"
          },
          "java": {
            "mode": "auto",
            "path": null
          },
          "memory": {
            "overrideGlobal": false,
            "minMb": null,
            "maxMb": null
          },
          "jvmArgs": {
            "preset": "default",
            "extra": []
          },
          "content": [
            {
              "provider": "modrinth",
              "projectId": "1KVo5zza",
              "versionId": "OuZrT7BT",
              "type": "modpack",
              "fileName": "Fabulously.Optimized-v14.1.2.mrpack",
              "sha1": "c1fdefaf40d788a281f99e8c486ff2d4dbef0883"
            }
          ],
          "createdAt": "2026-11-06T11:38:32.192Z",
          "lastPlayedAt": "2026-11-07T07:04:34.434Z",
          "iconUrl": "https://cdn.modrinth.com/data/1KVo5zza/d8152911f8fd5d7e9a8c499fe89045af81fe816e_96.webp"
        }"#;

        let instance: Result<Instance, _> = serde_json::from_str(json_str);
        assert!(instance.is_ok(), "Failed to parse: {:?}", instance.err());
        let inst = instance.unwrap();
        assert_eq!(inst.id, "0b4992bd-451f-447f-a028-d523b1284297");
        assert_eq!(inst.name, "Fabulously Optimized");
        assert_eq!(inst.loader.loader_type, launcher_types::LoaderType::Fabric);
        assert_eq!(inst.content.len(), 1);
        assert_eq!(
            inst.content[0].content_type,
            launcher_types::ContentType::Modpack
        );
    }

    #[test]
    fn test_cleanup_incomplete_modpacks_only_removes_marked_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let store = InstancesStore::new(paths.clone());

        let complete = store
            .create(CreateInstanceRequest {
                name: "Complete".to_string(),
                minecraft_version: "1.21.4".to_string(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Vanilla,
                    version: None,
                },
            })
            .unwrap();
        let incomplete = store
            .create(CreateInstanceRequest {
                name: "Interrupted Pack".to_string(),
                minecraft_version: "1.21.4".to_string(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Fabric,
                    version: Some("0.16.10".to_string()),
                },
            })
            .unwrap();

        fs::write(
            paths
                .instances_dir
                .join(&incomplete.id)
                .join(MODPACK_INSTALLING_MARKER),
            b"",
        )
        .unwrap();

        let removed = store.cleanup_incomplete_modpacks().unwrap();
        assert_eq!(removed, 1);
        assert!(paths.instances_dir.join(&complete.id).exists());
        assert!(!paths.instances_dir.join(&incomplete.id).exists());
    }

    #[test]
    fn test_update_validates_instance_icon_url() {
        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let store = InstancesStore::new(paths);

        let mut inst = store
            .create(CreateInstanceRequest {
                name: "IconTest".to_string(),
                minecraft_version: "1.21.4".to_string(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Vanilla,
                    version: None,
                },
            })
            .unwrap();

        inst.icon_url = Some("  https://cdn.modrinth.com/data/1/icon.webp  ".to_string());
        store.update(inst.clone()).unwrap();
        assert_eq!(
            store.get(&inst.id).unwrap().icon_url.as_deref(),
            Some("https://cdn.modrinth.com/data/1/icon.webp")
        );

        inst.icon_url = Some("data:image/png;base64,iVBORw0KGgo=".to_string());
        store.update(inst.clone()).unwrap();
        assert_eq!(
            store.get(&inst.id).unwrap().icon_url.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );

        inst.icon_url = Some("data:image/svg+xml;base64,PHN2Zz4=".to_string());
        assert!(store.update(inst.clone()).is_err());

        inst.icon_url = Some("javascript:alert(1)".to_string());
        assert!(store.update(inst.clone()).is_err());
    }
}
