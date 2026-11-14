use crate::fs_atomic::{read_json_file, write_json_atomic};
use chrono::Utc;
use launcher_platform::DataPaths;
use launcher_security::{slugify_instance_name, validate_instance_id};
use launcher_types::{
    CreateInstanceRequest, Instance, InstanceJavaSettings, InstanceJvmArgs, InstanceMemory,
    LauncherErrorPayload,
};
use std::fs;

const INSTANCE_MINECRAFT_DIRS: &[&str] = &[
    "mods",
    "saves",
    "shaderpacks",
    "resourcepacks",
    "config",
    "logs",
];

#[derive(Clone)]
pub struct InstancesStore {
    paths: DataPaths,
}

impl InstancesStore {
    pub fn new(paths: DataPaths) -> Self {
        Self { paths }
    }

    pub fn list(&self) -> Result<Vec<Instance>, LauncherErrorPayload> {
        let dir = &self.paths.instances_dir;
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(|e| {
                LauncherErrorPayload::internal(format!("Gagal membuat folder instances: {}", e))
            })?;
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(dir).map_err(|e| {
            LauncherErrorPayload::internal(format!("Gagal membaca folder instances: {}", e))
        })?;

        let mut instances = Vec::new();

        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let instance_file = entry.path().join("instance.json");
                if instance_file.exists() {
                    match read_json_file::<Instance>(&instance_file) {
                        Ok(Some(instance)) => instances.push(instance),
                        _ => {
                            tracing::warn!("Instance di {:?} korup atau tidak valid", entry.path());
                        }
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
        let instance_file = self.paths.instances_dir.join(id).join("instance.json");
        match read_json_file::<Instance>(&instance_file)? {
            Some(instance) => Ok(instance),
            None => Err(LauncherErrorPayload::not_found(format!(
                "Instance dengan ID '{}' tidak ditemukan.",
                id
            ))),
        }
    }

    pub fn create(&self, req: CreateInstanceRequest) -> Result<Instance, LauncherErrorPayload> {
        let trimmed_name = req.name.trim();
        if trimmed_name.is_empty() {
            return Err(LauncherErrorPayload::invalid_input(
                "Nama instance tidak boleh kosong.",
            ));
        }

        let base_id = slugify_instance_name(trimmed_name);
        let mut id = base_id.clone();
        let mut counter = 1;

        while self.paths.instances_dir.join(&id).exists() {
            id = format!("{}-{}", base_id, counter);
            counter += 1;
        }

        validate_instance_id(&id)?;

        let instance_dir = self.paths.instances_dir.join(&id);
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
    }

    pub fn update(&self, instance: Instance) -> Result<(), LauncherErrorPayload> {
        validate_instance_id(&instance.id)?;
        let instance_dir = self.paths.instances_dir.join(&instance.id);
        if !instance_dir.exists() {
            return Err(LauncherErrorPayload::not_found(format!(
                "Instance '{}' tidak ditemukan.",
                instance.id
            )));
        }

        let instance_file = instance_dir.join("instance.json");
        write_json_atomic(&instance_file, &instance)?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<(), LauncherErrorPayload> {
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
        if let Ok(existing) = self.get("default") {
            return Ok(existing);
        }

        self.create(CreateInstanceRequest {
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
}
