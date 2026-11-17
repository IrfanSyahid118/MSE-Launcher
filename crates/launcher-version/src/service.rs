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
    MinecraftVersionInfo, MinecraftVersionType, MojangDownloadArtifact, MojangVersionDetail,
    MojangVersionList,
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

    #[cfg(test)]
    pub(crate) fn new_loopback_test(paths: DataPaths) -> Self {
        Self {
            paths,
            http: HttpClient::new_loopback_test(),
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

    async fn fetch_verified_json_bytes<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        expected_sha1: Option<&str>,
        expected_size: Option<u64>,
        dest_root: &Path,
        dest_file: &Path,
    ) -> Result<T, LauncherErrorPayload> {
        let resp = self.http.get(url).await?;
        let bytes = resp.bytes().await.map_err(|e| {
            LauncherErrorPayload::network(format!("Gagal membaca respon dari {}: {}", url, e))
        })?;

        if let Some(size) = expected_size.filter(|&s| s > 0) {
            if bytes.len() as u64 != size {
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::HashMismatch,
                    format!(
                        "Ukuran berkas JSON tidak sesuai untuk {}. Diharapkan: {} byte, Didapat: {} byte",
                        url,
                        size,
                        bytes.len()
                    ),
                ));
            }
        }

        if let Some(sha1_expected) = expected_sha1.filter(|s| !s.trim().is_empty()) {
            use sha1::{Digest, Sha1};
            let actual = hex::encode(Sha1::digest(&bytes));
            if !actual.eq_ignore_ascii_case(sha1_expected) {
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::HashMismatch,
                    format!(
                        "Checksum SHA-1 tidak cocok untuk {}. Diharapkan: {}, Didapat: {}",
                        url, sha1_expected, actual
                    ),
                ));
            }
        }

        let parsed: T = serde_json::from_slice(&bytes).map_err(|e| {
            LauncherErrorPayload::network(format!(
                "Gagal mem-parse respon JSON dari {}: {}",
                url, e
            ))
        })?;

        let parent = dest_file.parent().ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Path tujuan cache JSON tidak memiliki parent.")
        })?;
        launcher_security::ensure_secure_dir(parent, true)?;
        launcher_security::ensure_descendants_no_symlink(dest_root, dest_file)?;

        let tmp = parent.join(format!(
            ".{}.tmp-{}",
            dest_file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("cache.json"),
            uuid::Uuid::new_v4().simple()
        ));
        {
            use std::io::Write;
            let mut file = launcher_security::safe_create_new_file(&tmp, true)?;
            if let Err(e) = file.write_all(&bytes) {
                let _ = fs::remove_file(&tmp);
                return Err(LauncherErrorPayload::file_system(format!(
                    "Gagal menulis cache JSON sementara: {}",
                    e
                )));
            }
            let _ = file.sync_all();
        }
        fs::rename(&tmp, dest_file).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            LauncherErrorPayload::file_system(format!("Gagal menyimpan cache JSON: {}", e))
        })?;

        Ok(parsed)
    }

    async fn fetch_manifest_cached(&self) -> Result<MojangVersionList, LauncherErrorPayload> {
        let cache_file = self.paths.cache_dir.join("version_manifest_v2.json");
        let _ =
            launcher_security::ensure_descendants_no_symlink(&self.paths.cache_dir, &cache_file);

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

        match self
            .http
            .get_json::<MojangVersionList>(MOJANG_VERSION_MANIFEST_URL)
            .await
        {
            Ok(fresh) => {
                let _ = write_json_atomic(&cache_file, &fresh);
                Ok(fresh)
            }
            Err(net_err) => {
                if cache_file.exists() {
                    if let Ok(Some(stale)) = read_json_file::<MojangVersionList>(&cache_file) {
                        tracing::warn!(
                            "Gagal menyegarkan manifest versi ({}); memakai cache lokal kedaluwarsa.",
                            net_err.message
                        );
                        return Ok(stale);
                    }
                }
                Err(net_err)
            }
        }
    }

    pub async fn get_version_detail(
        &self,
        version_id: &str,
    ) -> Result<MojangVersionDetail, LauncherErrorPayload> {
        launcher_security::validate_safe_single_component(version_id)?;
        let version_dir = launcher_security::safe_subpath(&self.paths.versions_dir, version_id)?;
        let detail_file =
            launcher_security::safe_subpath(&version_dir, &format!("{}.json", version_id))?;
        launcher_security::ensure_descendants_no_symlink(&self.paths.versions_dir, &detail_file)?;

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

        self.fetch_verified_json_bytes::<MojangVersionDetail>(
            &entry.url,
            Some(entry.sha1.as_str()),
            None,
            &self.paths.versions_dir,
            &detail_file,
        )
        .await
    }

    fn push_artifact_download_task(
        &self,
        tasks: &mut Vec<DownloadTask>,
        artifact: &MojangDownloadArtifact,
        lib_base_url: Option<&str>,
        rel_path: &str,
    ) -> Result<(), LauncherErrorPayload> {
        let dest = launcher_security::safe_subpath(&self.paths.libraries_dir, rel_path)?;
        let explicit_url = artifact.url.trim();
        let base_url = lib_base_url.map(str::trim).unwrap_or("");
        let missing_checksum = artifact.sha1.trim().is_empty();
        let size_matches = artifact.size == 0
            || dest
                .metadata()
                .map(|m| m.len() == artifact.size)
                .unwrap_or(false);

        if (explicit_url.is_empty() && base_url.is_empty())
            || (missing_checksum && dest.is_file() && size_matches)
        {
            return Ok(());
        }

        let raw_url = if !explicit_url.is_empty() {
            artifact.url.clone()
        } else {
            let base_clean = base_url.trim_end_matches('/');
            format!("{}/{}", base_clean, rel_path.trim_start_matches('/'))
        };
        let url = if let Some(rest) = raw_url.strip_prefix("http://") {
            format!("https://{}", rest)
        } else {
            raw_url
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
        Ok(())
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
                    self.push_artifact_download_task(
                        &mut tasks,
                        artifact,
                        lib.url.as_deref(),
                        artifact
                            .path
                            .as_deref()
                            .unwrap_or(&Self::maven_to_path(&lib.name)),
                    )?;
                }

                if let Some(ref classifiers) = downloads.classifiers {
                    // Only infer `natives-<os>` when `natives` map is absent; an explicit map omits unsupported OSes.
                    let native_key = match lib.natives.as_ref() {
                        Some(map) => map.get(current_os).map(|s| s.replace("${arch}", arch_bits)),
                        None => Some(format!("natives-{}", current_os)),
                    };

                    if let Some(native_key) = native_key {
                        if let Some(artifact) = classifiers.get(&native_key) {
                            let fallback_rel =
                                Self::maven_to_path(&format!("{}:{}", lib.name, native_key));
                            self.push_artifact_download_task(
                                &mut tasks,
                                artifact,
                                lib.url.as_deref(),
                                artifact.path.as_deref().unwrap_or(&fallback_rel),
                            )?;
                        }
                    }
                }
            } else {
                let rel_path = Self::maven_to_path(&lib.name);
                let dest = launcher_security::safe_subpath(&self.paths.libraries_dir, &rel_path)?;
                let has_sha1 = lib
                    .sha1
                    .as_deref()
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false);
                let expected_size = lib.size.filter(|&s| s > 0);
                let size_matches = expected_size
                    .map(|sz| dest.metadata().map(|m| m.len() == sz).unwrap_or(false))
                    .unwrap_or(true);
                if dest.is_file() && !has_sha1 && size_matches {
                    continue;
                }

                let base_url = lib
                    .url
                    .as_deref()
                    .filter(|u| !u.trim().is_empty())
                    .unwrap_or("https://libraries.minecraft.net/");
                let base_clean = base_url.trim_end_matches('/');
                let raw_url = format!("{}/{}", base_clean, rel_path.trim_start_matches('/'));
                let url = if let Some(rest) = raw_url.strip_prefix("http://") {
                    format!("https://{}", rest)
                } else {
                    raw_url
                };

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
        launcher_security::ensure_descendants_no_symlink(&self.paths.assets_dir, &index_file)?;

        let expected_sha1 =
            (!index_info.sha1.trim().is_empty()).then_some(index_info.sha1.as_str());
        let expected_size = (index_info.size > 0).then_some(index_info.size);

        let cache_valid = index_file.exists()
            && launcher_download::verify_local_file(&index_file, expected_size, expected_sha1)
                .await;

        let index = if cache_valid {
            match read_json_file::<MojangAssetIndex>(&index_file)? {
                Some(idx) => idx,
                None => {
                    self.fetch_verified_json_bytes::<MojangAssetIndex>(
                        &index_info.url,
                        expected_sha1,
                        expected_size,
                        &self.paths.assets_dir,
                        &index_file,
                    )
                    .await?
                }
            }
        } else {
            self.fetch_verified_json_bytes::<MojangAssetIndex>(
                &index_info.url,
                expected_sha1,
                expected_size,
                &self.paths.assets_dir,
                &index_file,
            )
            .await?
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

        let osx_tasks = service
            .build_version_download_tasks_for_platform(&detail, "osx", "arm64", "64")
            .unwrap();
        assert!(
            !osx_tasks.iter().any(|t| t.url.contains("natives-")),
            "natives classifier must be skipped when explicit natives map omits current_os"
        );
    }

    #[tokio::test]
    async fn test_h13_verified_json_rejects_sha1_mismatch() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut req = [0u8; 512];
                let _ = stream.read(&mut req).await;
                let body = b"{}";
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(body).await;
                let _ = stream.shutdown().await;
            }
        });

        let temp = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(temp.path());
        let service = VersionService::new_loopback_test(paths.clone());
        let dest_file = paths.versions_dir.join("1.20.1").join("1.20.1.json");
        let err = service
            .fetch_verified_json_bytes::<MojangVersionDetail>(
                &format!("http://{}/detail.json", addr),
                Some("0000000000000000000000000000000000000000"),
                None,
                &paths.versions_dir,
                &dest_file,
            )
            .await
            .expect_err("Corrupted version detail JSON must be rejected");
        server.abort();
        assert_eq!(err.code, launcher_types::ErrorCode::HashMismatch);
        assert!(!dest_file.exists());
    }
}
