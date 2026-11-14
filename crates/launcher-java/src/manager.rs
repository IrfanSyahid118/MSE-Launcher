use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use launcher_download::{DownloadManager, DownloadProgress, DownloadTask};
use launcher_net::HttpClient;
use launcher_platform::DataPaths;
use launcher_types::{ErrorCode, JavaInstallation, LauncherErrorPayload};
use serde::{Deserialize, Serialize};

use crate::detector::{parse_java_output, run_java_version};

pub const RUNTIME_CATALOG_URL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[cfg(target_os = "windows")]
const PLATFORM_KEY: &str = "windows-x64";
#[cfg(target_os = "linux")]
const PLATFORM_KEY: &str = "linux";
#[cfg(target_os = "macos")]
const PLATFORM_KEY: &str = "mac-os";
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
const PLATFORM_KEY: &str = "linux";

#[cfg(windows)]
const JAVA_BIN_NAME: &str = "java.exe";
#[cfg(not(windows))]
const JAVA_BIN_NAME: &str = "java";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifestRef {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCatalogEntry {
    pub manifest: RuntimeManifestRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeFileDownload {
    pub raw: RuntimeManifestRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuntimeFileEntry {
    #[serde(rename = "file")]
    File {
        downloads: RuntimeFileDownload,
        executable: Option<bool>,
    },
    #[serde(rename = "directory")]
    Directory,
    #[serde(rename = "link")]
    Link { target: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifest {
    pub files: HashMap<String, RuntimeFileEntry>,
}

pub(crate) fn resolve_manifest_rel_path(
    target_dir: &Path,
    rel_path: &str,
) -> Result<PathBuf, LauncherErrorPayload> {
    launcher_security::safe_subpath(target_dir, rel_path).map_err(|_| {
        LauncherErrorPayload::new(
            ErrorCode::InvalidInput,
            format!("Manifest Java mengandung path tidak aman: {rel_path}"),
        )
    })
}

pub fn component_for_major(major: u32) -> &'static str {
    match major {
        8 => "jre-legacy",
        16 => "java-runtime-alpha",
        17 => "java-runtime-gamma",
        21 => "java-runtime-delta",
        25.. => "java-runtime-epsilon",
        _ => "java-runtime-delta",
    }
}

pub async fn ensure_managed_java<F>(
    paths: &DataPaths,
    major: u32,
    component_override: Option<&str>,
    http: &HttpClient,
    downloader: &DownloadManager,
    progress_callback: F,
    cancelled: Arc<AtomicBool>,
) -> Result<JavaInstallation, LauncherErrorPayload>
where
    F: Fn(DownloadProgress) + Send + Sync + 'static,
{
    ensure_managed_java_with_probe(
        paths,
        major,
        component_override,
        http,
        downloader,
        progress_callback,
        cancelled,
        RUNTIME_CATALOG_URL,
        run_java_version,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn ensure_managed_java_with_probe<F, P>(
    paths: &DataPaths,
    major: u32,
    component_override: Option<&str>,
    http: &HttpClient,
    downloader: &DownloadManager,
    progress_callback: F,
    cancelled: Arc<AtomicBool>,
    catalog_url: &str,
    probe: P,
) -> Result<JavaInstallation, LauncherErrorPayload>
where
    F: Fn(DownloadProgress) + Send + Sync + 'static,
    P: Fn(&Path) -> Result<String, LauncherErrorPayload>,
{
    let default_comp = component_for_major(major);
    let component = component_override.unwrap_or(default_comp);
    launcher_security::validate_safe_single_component(component)?;
    let platform_dir = paths.runtimes_dir.join(PLATFORM_KEY);
    let target_dir = launcher_security::safe_subpath(&platform_dir, component)?;
    let java_exe = target_dir.join("bin").join(JAVA_BIN_NAME);

    if java_exe.exists() {
        if let Ok(output) = probe(&java_exe) {
            if let Ok(inst) = parse_java_output(&output, &java_exe, paths) {
                if inst.major_version == major {
                    return Ok(inst);
                }
                tracing::warn!(
                    "Java managed di {:?} memiliki major version {} (dibutuhkan {}), mengunduh ulang.",
                    java_exe,
                    inst.major_version,
                    major
                );
            }
        }
    }

    let catalog: HashMap<String, HashMap<String, Vec<RuntimeCatalogEntry>>> =
        http.get_json(catalog_url).await?;

    let platform_catalog = catalog.get(PLATFORM_KEY).ok_or_else(|| {
        LauncherErrorPayload::not_found(format!(
            "Platform {} tidak ada di katalog Mojang.",
            PLATFORM_KEY
        ))
    })?;

    let entries = platform_catalog.get(component).ok_or_else(|| {
        LauncherErrorPayload::not_found(format!(
            "Komponen runtime '{}' tidak ditemukan.",
            component
        ))
    })?;

    let first = entries.first().ok_or_else(|| {
        LauncherErrorPayload::not_found(format!("Entri manifest kosong untuk {}", component))
    })?;

    let manifest: RuntimeManifest = http.get_json(&first.manifest.url).await?;

    let mut tasks = Vec::new();

    for (rel_path, entry) in &manifest.files {
        match entry {
            RuntimeFileEntry::File { downloads, .. } => {
                let dest = resolve_manifest_rel_path(&target_dir, rel_path)?;
                tasks.push(DownloadTask {
                    url: downloads.raw.url.clone(),
                    destination: dest,
                    expected_sha1: Some(downloads.raw.sha1.clone()),
                    expected_sha256: None,
                    expected_size: Some(downloads.raw.size),
                });
            }
            RuntimeFileEntry::Directory => {
                let dir = resolve_manifest_rel_path(&target_dir, rel_path)?;
                launcher_security::ensure_descendants_no_symlink(&target_dir, &dir)?;
                let _ = std::fs::create_dir_all(&dir);
            }
            RuntimeFileEntry::Link { target } => {
                #[cfg(unix)]
                {
                    let link_path = resolve_manifest_rel_path(&target_dir, rel_path)?;
                    launcher_security::validate_relative_symlink_target_within(
                        &target_dir,
                        &link_path,
                        target,
                    )?;
                    if let Some(parent) = link_path.parent() {
                        launcher_security::ensure_descendants_no_symlink(&target_dir, parent)?;
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::os::unix::fs::symlink(target, &link_path);
                }
                #[cfg(not(unix))]
                let _ = target;
            }
        }
    }

    downloader
        .download_all(tasks, progress_callback, cancelled)
        .await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for (rel_path, entry) in &manifest.files {
            if let RuntimeFileEntry::File {
                executable: Some(true),
                ..
            } = entry
            {
                let p = resolve_manifest_rel_path(&target_dir, rel_path)?;
                if let Ok(meta) = std::fs::symlink_metadata(&p) {
                    if meta.file_type().is_file() && !meta.file_type().is_symlink() {
                        let _ =
                            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755));
                    }
                }
            }
        }
        if let Ok(meta) = std::fs::symlink_metadata(&java_exe) {
            if meta.file_type().is_file() && !meta.file_type().is_symlink() {
                let _ = std::fs::set_permissions(&java_exe, std::fs::Permissions::from_mode(0o755));
            }
        }
    }

    let output = probe(&java_exe)?;
    parse_java_output(&output, &java_exe, paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_n24_ensure_managed_java_rejects_mismatched_cached_major_version() {
        let tmp = tempdir().unwrap();
        let paths = DataPaths::new(tmp.path());
        let component = component_for_major(21);
        let java_exe = paths
            .runtimes_dir
            .join(PLATFORM_KEY)
            .join(component)
            .join("bin")
            .join(JAVA_BIN_NAME);
        std::fs::create_dir_all(java_exe.parent().unwrap()).unwrap();
        std::fs::write(&java_exe, b"fake").unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            if let Ok((mut s, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut s, &mut buf).await;
                let _ = s
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                    .await;
            }
        });

        let http = HttpClient::default();
        let dm = DownloadManager::new(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let catalog_url = format!("http://{}/catalog.json", addr);

        let res = ensure_managed_java_with_probe(
            &paths,
            21,
            None,
            &http,
            &dm,
            |_| {},
            cancel,
            &catalog_url,
            |_| {
                Ok(
                    "java.vendor = Eclipse Adoptium\nos.arch = amd64\nsun.arch.data.model = 64\nopenjdk version \"17.0.8\" 2023-07-18"
                        .to_string(),
                )
            },
        )
        .await;
        server.abort();

        assert!(
            res.is_err(),
            "ensure_managed_java must not return cached Java 17 when major 21 is requested"
        );
    }
}
