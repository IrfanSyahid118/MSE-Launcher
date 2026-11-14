use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use launcher_download::DownloadTask;
use launcher_net::HttpClient;
use launcher_platform::DataPaths;
use launcher_storage::{read_json_file, write_json_atomic};
use launcher_types::LauncherErrorPayload;

use crate::assets::{
    build_asset_download_tasks, prepare_legacy_assets_from_index, MojangAssetIndex,
};
use crate::models::{
    MinecraftVersionInfo, MinecraftVersionType, MojangVersionDetail, MojangVersionList,
};
use crate::rules::is_library_allowed;

pub const MOJANG_VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

const MANIFEST_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone)]
pub struct VersionService {
    paths: DataPaths,
    http: HttpClient,
}

impl VersionService {
    pub fn new(paths: DataPaths) -> Self {
        Self {
            paths,
            http: HttpClient::new(),
        }
    }

    pub async fn list_versions(&self) -> Result<Vec<MinecraftVersionInfo>, LauncherErrorPayload> {
        let manifest = self.fetch_manifest_cached().await?;

        let installed_versions = self.scan_installed_versions();

        let infos = manifest
            .versions
            .into_iter()
            .map(|v| {
                let v_type = match v.version_type.as_str() {
                    "release" => MinecraftVersionType::Release,
                    "snapshot" => MinecraftVersionType::Snapshot,
                    "old_beta" => MinecraftVersionType::OldBeta,
                    _ => MinecraftVersionType::OldAlpha,
                };
                let installed = installed_versions.contains(&v.id);

                MinecraftVersionInfo {
                    id: v.id,
                    version_type: v_type,
                    release_time: v.release_time,
                    installed,
                }
            })
            .collect();

        Ok(infos)
    }

    fn scan_installed_versions(&self) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        let versions_dir = &self.paths.versions_dir;

        if let Ok(entries) = fs::read_dir(versions_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() && !file_type.is_symlink() {
                        let id = entry.file_name().to_string_lossy().to_string();
                        if launcher_security::validate_safe_single_component(&id).is_ok() {
                            let json_file = entry.path().join(format!("{}.json", id));
                            if json_file.exists() {
                                set.insert(id);
                            }
                        }
                    }
                }
            }
        }

        set
    }

    async fn fetch_manifest_cached(&self) -> Result<MojangVersionList, LauncherErrorPayload> {
        let cache_file = self.paths.cache_dir.join("version_manifest_v2.json");

        if cache_file.exists() {
            if let Ok(metadata) = fs::metadata(&cache_file) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                        if elapsed < MANIFEST_CACHE_TTL {
                            if let Ok(Some(cached)) =
                                read_json_file::<MojangVersionList>(&cache_file)
                            {
                                return Ok(cached);
                            }
                        }
                    }
                }
            }
        }

        let fresh = self
            .http
            .get_json::<MojangVersionList>(MOJANG_VERSION_MANIFEST_URL)
            .await?;
        let _ = write_json_atomic(&cache_file, &fresh);
        Ok(fresh)
    }

    pub async fn get_version_detail(
        &self,
        version_id: &str,
    ) -> Result<MojangVersionDetail, LauncherErrorPayload> {
        launcher_security::validate_safe_single_component(version_id)?;
        let version_dir = launcher_security::safe_subpath(&self.paths.versions_dir, version_id)?;
        let detail_file =
            launcher_security::safe_subpath(&version_dir, &format!("{}.json", version_id))?;

        if detail_file.exists() {
            if let Ok(Some(detail)) = read_json_file::<MojangVersionDetail>(&detail_file) {
                return Ok(detail);
            }
        }

        let manifest = self.fetch_manifest_cached().await?;
        let entry = manifest
            .versions
            .iter()
            .find(|v| v.id == version_id)
            .ok_or_else(|| {
                LauncherErrorPayload::not_found(format!(
                    "Versi Minecraft '{}' tidak ditemukan di manifest resmi Mojang.",
                    version_id
                ))
            })?;

        let detail = self
            .http
            .get_json::<MojangVersionDetail>(&entry.url)
            .await?;
        write_json_atomic(&detail_file, &detail)?;

        Ok(detail)
    }

    pub fn build_version_download_tasks(
        &self,
        detail: &MojangVersionDetail,
    ) -> Result<Vec<DownloadTask>, LauncherErrorPayload> {
        self.build_version_download_tasks_for_platform(
            detail,
            crate::rules::host_os(),
            crate::rules::host_arch(),
            crate::rules::host_arch_bits(),
        )
    }

    pub(crate) fn build_version_download_tasks_for_platform(
        &self,
        detail: &MojangVersionDetail,
        current_os: &str,
        current_arch: &str,
        arch_bits: &str,
    ) -> Result<Vec<DownloadTask>, LauncherErrorPayload> {
        let mut tasks = Vec::new();

        let client_version_id = detail.inherits_from.as_deref().unwrap_or(&detail.id);
        launcher_security::validate_safe_single_component(client_version_id)?;
        if let Some(ref downloads) = detail.downloads {
            if let Some(ref client) = downloads.client {
                let version_dir =
                    launcher_security::safe_subpath(&self.paths.versions_dir, client_version_id)?;
                let dest = launcher_security::safe_subpath(
                    &version_dir,
                    &format!("{}.jar", client_version_id),
                )?;

                tasks.push(DownloadTask {
                    url: client.url.clone(),
                    destination: dest,
                    expected_sha1: if client.sha1.is_empty() {
                        None
                    } else {
                        Some(client.sha1.clone())
                    },
                    expected_sha256: None,
                    expected_size: if client.size == 0 {
                        None
                    } else {
                        Some(client.size)
                    },
                });
            }
        }

        for lib in &detail.libraries {
            if !is_library_allowed(lib.rules.as_deref(), current_os, current_arch) {
                continue;
            }

            if let Some(ref downloads) = lib.downloads {
                if let Some(ref artifact) = downloads.artifact {
                    let rel_path = artifact
                        .path
                        .clone()
                        .unwrap_or_else(|| Self::maven_to_path(&lib.name));

                    let dest =
                        launcher_security::safe_subpath(&self.paths.libraries_dir, &rel_path)?;

                    let url = if !artifact.url.trim().is_empty() {
                        artifact.url.clone()
                    } else {
                        let base_url = lib
                            .url
                            .as_deref()
                            .filter(|u| !u.trim().is_empty())
                            .unwrap_or("https://libraries.minecraft.net/");
                        let base_clean = base_url.trim_end_matches('/');
                        format!("{}/{}", base_clean, rel_path.trim_start_matches('/'))
                    };

                    tasks.push(DownloadTask {
                        url,
                        destination: dest,
                        expected_sha1: if artifact.sha1.is_empty() {
                            None
                        } else {
                            Some(artifact.sha1.clone())
                        },
                        expected_sha256: None,
                        expected_size: if artifact.size == 0 {
                            None
                        } else {
                            Some(artifact.size)
                        },
                    });
                }

                if let Some(ref classifiers) = downloads.classifiers {
                    let native_key = lib
                        .natives
                        .as_ref()
                        .and_then(|n| n.get(current_os))
                        .map(|s| s.replace("${arch}", arch_bits))
                        .unwrap_or_else(|| format!("natives-{}", current_os));

                    if let Some(artifact) = classifiers.get(&native_key) {
                        let rel_path = artifact.path.clone().unwrap_or_else(|| {
                            Self::maven_to_path(&format!("{}:{}", lib.name, native_key))
                        });

                        let dest =
                            launcher_security::safe_subpath(&self.paths.libraries_dir, &rel_path)?;

                        let url = if !artifact.url.trim().is_empty() {
                            artifact.url.clone()
                        } else {
                            let base_url = lib
                                .url
                                .as_deref()
                                .filter(|u| !u.trim().is_empty())
                                .unwrap_or("https://libraries.minecraft.net/");
                            let base_clean = base_url.trim_end_matches('/');
                            format!("{}/{}", base_clean, rel_path.trim_start_matches('/'))
                        };

                        tasks.push(DownloadTask {
                            url,
                            destination: dest,
                            expected_sha1: if artifact.sha1.is_empty() {
                                None
                            } else {
                                Some(artifact.sha1.clone())
                            },
                            expected_sha256: None,
                            expected_size: if artifact.size == 0 {
                                None
                            } else {
                                Some(artifact.size)
                            },
                        });
                    }
                }
            } else {
                let rel_path = Self::maven_to_path(&lib.name);
                let dest = launcher_security::safe_subpath(&self.paths.libraries_dir, &rel_path)?;

                let base_url = lib
                    .url
                    .as_deref()
                    .filter(|u| !u.trim().is_empty())
                    .unwrap_or("https://libraries.minecraft.net/");
                let base_clean = base_url.trim_end_matches('/');
                let url = format!("{}/{}", base_clean, rel_path.trim_start_matches('/'));

                tasks.push(DownloadTask {
                    url,
                    destination: dest,
                    expected_sha1: lib.sha1.clone().filter(|s| !s.is_empty()),
                    expected_sha256: None,
                    expected_size: lib.size.filter(|&s| s > 0),
                });
            }
        }

        Ok(tasks)
    }

    async fn load_asset_index(
        &self,
        detail: &MojangVersionDetail,
    ) -> Result<Option<(String, MojangAssetIndex)>, LauncherErrorPayload> {
        let index_info = match detail.asset_index {
            Some(ref info) => info,
            None => return Ok(None),
        };

        launcher_security::validate_safe_single_component(&index_info.id)?;
        let indexes_dir = self.paths.assets_dir.join("indexes");
        let index_file =
            launcher_security::safe_subpath(&indexes_dir, &format!("{}.json", index_info.id))?;

        let index = if index_file.exists() {
            match read_json_file::<MojangAssetIndex>(&index_file)? {
                Some(idx) => idx,
                None => {
                    let fresh = self
                        .http
                        .get_json::<MojangAssetIndex>(&index_info.url)
                        .await?;
                    write_json_atomic(&index_file, &fresh)?;
                    fresh
                }
            }
        } else {
            let fresh = self
                .http
                .get_json::<MojangAssetIndex>(&index_info.url)
                .await?;
            write_json_atomic(&index_file, &fresh)?;
            fresh
        };

        Ok(Some((index_info.id.clone(), index)))
    }

    pub async fn fetch_asset_tasks(
        &self,
        detail: &MojangVersionDetail,
    ) -> Result<Vec<DownloadTask>, LauncherErrorPayload> {
        let Some((_, index)) = self.load_asset_index(detail).await? else {
            return Ok(Vec::new());
        };
        Ok(build_asset_download_tasks(&index, &self.paths.assets_dir))
    }

    pub async fn prepare_game_assets(
        &self,
        detail: &MojangVersionDetail,
        game_dir: &Path,
    ) -> Result<PathBuf, LauncherErrorPayload> {
        let Some((index_id, index)) = self.load_asset_index(detail).await? else {
            return Ok(self.paths.assets_dir.clone());
        };
        prepare_legacy_assets_from_index(&index, &index_id, &self.paths.assets_dir, game_dir)
    }

    /// Mengubah maven coordinate (misal: "com.mojang:patchy:1.1") menjadi path berkas
    pub fn maven_to_path(coordinate: &str) -> String {
        let (coord_without_ext, extension) = if let Some(idx) = coordinate.rfind('@') {
            (&coordinate[..idx], &coordinate[idx + 1..])
        } else {
            (coordinate, "jar")
        };

        let parts: Vec<&str> = coord_without_ext.split(':').collect();
        if parts.len() < 3 {
            return coordinate.replace(':', "/");
        }

        let group = parts.first().copied().unwrap_or("").replace('.', "/");
        let artifact = parts.get(1).copied().unwrap_or("");
        let version = parts.get(2).copied().unwrap_or("");
        let classifier = parts.get(3).map(|c| format!("-{}", c)).unwrap_or_default();

        format!(
            "{}/{}/{}/{}-{}{}.{}",
            group, artifact, version, artifact, version, classifier, extension
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    #[test]
    fn test_deserialize_version_detail() {
        let json_str = r#"{
            "id": "1.20.1",
            "inheritsFrom": "1.20",
            "mainClass": "net.minecraft.client.main.Main",
            "libraries": [
                {
                    "name": "com.mojang:patchy:1.1",
                    "url": "https://libraries.minecraft.net/"
                }
            ]
        }"#;
        let detail: MojangVersionDetail = serde_json::from_str(json_str).unwrap();
        assert_eq!(detail.id, "1.20.1");
        assert_eq!(detail.inherits_from.as_deref(), Some("1.20"));
        assert_eq!(detail.libraries.len(), 1);
        assert_eq!(detail.libraries[0].name, "com.mojang:patchy:1.1");
    }

    #[test]
    fn test_maven_to_path() {
        assert_eq!(
            VersionService::maven_to_path("net.fabricmc:fabric-loader:0.20.1"),
            "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"
        );
        assert_eq!(
            VersionService::maven_to_path("com.mojang:patchy:1.1@jar"),
            "com/mojang/patchy/1.1/patchy-1.1.jar"
        );
        assert_eq!(
            VersionService::maven_to_path("org.lwjgl:lwjgl:3.3.1:natives-windows"),
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
        );
    }

    #[test]
    fn test_build_download_tasks_loader() {
        let paths = DataPaths::default_from_env();
        let service = VersionService::new(paths);
        let detail = MojangVersionDetail {
            id: "fabric-loader-0.16.0-1.20.1".to_string(),
            inherits_from: Some("1.20.1".to_string()),
            downloads: Some(MojangDownloads {
                client: Some(MojangDownloadArtifact {
                    path: None,
                    sha1: "abc123".to_string(),
                    size: 1024,
                    url: "https://example.com/client.jar".to_string(),
                }),
                server: None,
                client_mappings: None,
            }),
            libraries: vec![
                MojangLibrary {
                    name: "net.fabricmc:fabric-loader:0.16.0".to_string(),
                    url: Some("https://maven.fabricmc.net/".to_string()),
                    ..Default::default()
                },
                MojangLibrary {
                    name: "org.ow2.asm:asm:9.6".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let tasks = service.build_version_download_tasks(&detail).unwrap();
        assert_eq!(tasks.len(), 3);
        assert!(tasks[0].destination.ends_with("1.20.1.jar"));
        assert_eq!(
            tasks[1].url,
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar"
        );
        assert_eq!(
            tasks[2].url,
            "https://libraries.minecraft.net/org/ow2/asm/asm/9.6/asm-9.6.jar"
        );
    }

    #[test]
    fn test_n26_build_download_tasks_respects_target_platform() {
        use crate::{MojangRule, MojangRuleOs};
        use std::collections::HashMap;

        let paths = DataPaths::default_from_env();
        let service = VersionService::new(paths);

        let mut linux_classifiers = HashMap::new();
        linux_classifiers.insert(
            "natives-linux".to_string(),
            MojangDownloadArtifact {
                path: Some("org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar".to_string()),
                sha1: String::new(),
                size: 100,
                url: "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar".to_string(),
            },
        );
        linux_classifiers.insert(
            "natives-windows".to_string(),
            MojangDownloadArtifact {
                path: Some("org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar".to_string()),
                sha1: String::new(),
                size: 100,
                url: "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar".to_string(),
            },
        );

        let mut natives_map = HashMap::new();
        natives_map.insert("linux".to_string(), "natives-linux".to_string());
        natives_map.insert("windows".to_string(), "natives-windows".to_string());

        let detail = MojangVersionDetail {
            id: "1.20.1".to_string(),
            libraries: vec![
                MojangLibrary {
                    name: "com.mojang:text2speech:1.17.9".to_string(),
                    rules: Some(vec![MojangRule {
                        action: "allow".to_string(),
                        os: Some(MojangRuleOs {
                            name: Some("linux".to_string()),
                            arch: None,
                            version: None,
                        }),
                        features: None,
                    }]),
                    ..Default::default()
                },
                MojangLibrary {
                    name: "org.lwjgl:lwjgl:3.3.1".to_string(),
                    downloads: Some(crate::models::MojangLibraryDownloads {
                        artifact: None,
                        classifiers: Some(linux_classifiers),
                    }),
                    natives: Some(natives_map),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let linux_tasks = service
            .build_version_download_tasks_for_platform(&detail, "linux", "x64", "64")
            .unwrap();
        assert!(
            linux_tasks.iter().any(|t| t.url.contains("text2speech")),
            "linux-only library rule must be allowed for linux platform"
        );
        assert!(
            linux_tasks.iter().any(|t| t.url.contains("natives-linux")),
            "natives-linux classifier must be downloaded for linux platform"
        );
        assert!(
            !linux_tasks
                .iter()
                .any(|t| t.url.contains("natives-windows")),
            "natives-windows classifier must not be selected for linux platform"
        );
    }
}
