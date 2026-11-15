use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use launcher_archive::extract_zip;
use launcher_download::{DownloadManager, DownloadTask};
use launcher_net::HttpClient;
use launcher_platform::DataPaths;
use launcher_storage::InstancesStore;
use launcher_types::{
    ContentProviderId, ContentType, CreateInstanceRequest, ErrorCode, Instance,
    InstanceContentEntry, LauncherErrorPayload, LoaderSpec, LoaderType,
};
use serde::Deserialize;

use crate::curseforge::{
    get_curseforge_file_with_base, get_curseforge_project_with_base,
    to_content_version as cf_to_version, API_BASE as CF_API_BASE,
};
use crate::modrinth::{
    get_modrinth_project_with_base, get_modrinth_version_with_base,
    to_content_version as mr_to_version, API_BASE as MR_API_BASE,
};

#[derive(Debug, Clone, Default)]
pub struct ModpackInstallOptions {
    pub project_id: Option<String>,
    pub icon_url: Option<String>,
}

struct ResolvedModpackMeta {
    project_id: Option<String>,
    version_id: String,
    file_name: String,
    sha1: String,
    icon_url: Option<String>,
}

struct InstanceRollbackGuard<'a> {
    instances: &'a InstancesStore,
    instances_dir: &'a Path,
    instance_id: Option<String>,
}

impl InstanceRollbackGuard<'_> {
    fn disarm(&mut self) {
        self.instance_id = None;
    }
}

impl Drop for InstanceRollbackGuard<'_> {
    fn drop(&mut self) {
        if let Some(ref id) = self.instance_id {
            let _ = self.instances.remove(id);
            let _ = launcher_security::safe_remove_dir_all_within(
                self.instances_dir,
                &self.instances_dir.join(id),
            );
        }
    }
}

fn build_modpack_content_entry(
    provider: ContentProviderId,
    meta: &ResolvedModpackMeta,
) -> Option<InstanceContentEntry> {
    let project_id = meta
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    Some(InstanceContentEntry {
        provider,
        project_id: project_id.to_string(),
        version_id: meta.version_id.clone(),
        content_type: ContentType::Modpack,
        file_name: meta.file_name.clone(),
        sha1: meta.sha1.clone(),
        world_folder: None,
        disabled: Some(false),
    })
}

fn create_pending_modpack_instance<'a>(
    paths: &'a DataPaths,
    instances: &'a InstancesStore,
    name: &str,
    minecraft_version: String,
    loader: LoaderSpec,
    provider: ContentProviderId,
    meta: &ResolvedModpackMeta,
) -> Result<
    (
        Instance,
        InstanceRollbackGuard<'a>,
        Option<InstanceContentEntry>,
    ),
    LauncherErrorPayload,
> {
    let mut new_inst = instances.create(CreateInstanceRequest {
        name: name.to_string(),
        minecraft_version,
        loader,
    })?;
    let guard = InstanceRollbackGuard {
        instances,
        instances_dir: &paths.instances_dir,
        instance_id: Some(new_inst.id.clone()),
    };

    let marker_path = paths
        .instances_dir
        .join(&new_inst.id)
        .join(launcher_storage::MODPACK_INSTALLING_MARKER);
    fs::write(&marker_path, b"").map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal menulis penanda instalasi modpack: {}", e))
    })?;

    let modpack_entry = build_modpack_content_entry(provider, meta);
    if modpack_entry.is_some() || meta.icon_url.is_some() {
        new_inst.icon_url = meta.icon_url.clone();
        if let Some(ref entry) = modpack_entry {
            new_inst.content = vec![entry.clone()];
        }
        instances.update(new_inst.clone())?;
    }

    Ok((new_inst, guard, modpack_entry))
}

fn sanitize_icon_url(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(trimmed).ok()?;
    match parsed.scheme() {
        "https" => Some(trimmed.to_string()),
        #[cfg(any(test, debug_assertions))]
        "http" => {
            let host = parsed.host_str()?.to_ascii_lowercase();
            matches!(host.as_str(), "localhost" | "127.0.0.1").then(|| trimmed.to_string())
        }
        _ => None,
    }
}

fn is_allowed_mrpack_download_url(raw: &str) -> bool {
    let Ok(parsed) = url::Url::parse(raw.trim()) else {
        return false;
    };
    let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    match parsed.scheme() {
        "https" => matches!(
            host.as_str(),
            "cdn.modrinth.com"
                | "github.com"
                | "raw.githubusercontent.com"
                | "objects.githubusercontent.com"
                | "release-assets.githubusercontent.com"
                | "gitlab.com"
                | "bitbucket.org"
                | "edge.forgecdn.net"
                | "mediafilez.forgecdn.net"
        ),
        #[cfg(any(test, debug_assertions))]
        "http" => matches!(host.as_str(), "localhost" | "127.0.0.1"),
        _ => false,
    }
}

fn is_valid_sha1_hex(value: Option<&str>) -> bool {
    value
        .map(str::trim)
        .is_some_and(|hex| hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[derive(Debug, Deserialize)]
struct ModrinthIndexFile {
    path: String,
    hashes: HashMap<String, String>,
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ModrinthIndex {
    name: String,
    dependencies: HashMap<String, String>,
    files: Vec<ModrinthIndexFile>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeManifestLoader {
    id: String,
    primary: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeManifestMinecraft {
    version: String,
    #[serde(rename = "modLoaders", default)]
    mod_loaders: Vec<CurseForgeManifestLoader>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
}

#[derive(Debug, Deserialize)]
struct CurseForgeManifest {
    name: String,
    minecraft: CurseForgeManifestMinecraft,
    #[serde(default)]
    files: Vec<CurseForgeManifestFile>,
    #[serde(default = "default_overrides")]
    overrides: String,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

fn parse_loader_from_modrinth(deps: &HashMap<String, String>) -> LoaderSpec {
    if let Some(v) = deps.get("fabric-loader") {
        LoaderSpec {
            loader_type: LoaderType::Fabric,
            version: Some(v.clone()),
        }
    } else if let Some(v) = deps.get("quilt-loader") {
        LoaderSpec {
            loader_type: LoaderType::Quilt,
            version: Some(v.clone()),
        }
    } else if let Some(v) = deps.get("neoforge") {
        LoaderSpec {
            loader_type: LoaderType::Neoforge,
            version: Some(v.clone()),
        }
    } else if let Some(v) = deps.get("forge") {
        LoaderSpec {
            loader_type: LoaderType::Forge,
            version: Some(v.clone()),
        }
    } else {
        LoaderSpec {
            loader_type: LoaderType::Vanilla,
            version: None,
        }
    }
}

fn parse_loader_from_curseforge(loader_str: &str) -> LoaderSpec {
    let lower = loader_str.to_lowercase();
    if let Some((kind, ver)) = lower.split_once('-') {
        let loader_type = match kind {
            "fabric" => LoaderType::Fabric,
            "quilt" => LoaderType::Quilt,
            "neoforge" => LoaderType::Neoforge,
            "forge" => LoaderType::Forge,
            _ => LoaderType::Vanilla,
        };
        LoaderSpec {
            loader_type,
            version: Some(ver.to_string()),
        }
    } else {
        LoaderSpec {
            loader_type: LoaderType::Vanilla,
            version: None,
        }
    }
}

fn read_zip_text(
    zip_path: &Path,
    entry_name: &str,
) -> Result<Option<String>, LauncherErrorPayload> {
    let file = File::open(zip_path).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal membuka arsip modpack: {}", e))
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal membaca ZIP modpack: {}", e))
    })?;

    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal membaca entry ZIP: {}", e))
        })?;
        if entry
            .name()
            .is_ok_and(|n| n.eq_ignore_ascii_case(entry_name))
        {
            let mut contents = String::new();
            entry
                .take(10 * 1024 * 1024)
                .read_to_string(&mut contents)
                .map_err(|e| {
                    LauncherErrorPayload::file_system(format!(
                        "Gagal membaca teks entry ZIP: {}",
                        e
                    ))
                })?;
            return Ok(Some(contents));
        }
    }

    Ok(None)
}

fn copy_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(dst) {
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Symlink tidak diizinkan pada direktori tujuan modpack",
            ));
        }
    }
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    let canon_dst = dunce::canonicalize(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Symlink tidak diizinkan dalam folder overrides modpack",
            ));
        }
        let name = entry.file_name();
        let name_str = name.to_str().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Nama berkas overrides bukan UTF-8 yang valid",
            )
        })?;
        launcher_security::validate_single_filename(name_str)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.message))?;
        let src_child = entry.path();
        let dst_child = dst.join(name_str);
        if ft.is_dir() {
            copy_recursive(&src_child, &dst_child)?;
        } else if ft.is_file() {
            if let Ok(existing_meta) = fs::symlink_metadata(&dst_child) {
                if existing_meta.file_type().is_symlink() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Menimpa symlink pada overrides tidak diizinkan",
                    ));
                }
            }
            if !dunce::canonicalize(dst)?.starts_with(&canon_dst) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Target salin overrides keluar batas direktori",
                ));
            }
            fs::copy(&src_child, &dst_child)?;
        }
    }
    Ok(())
}

pub(crate) fn safe_modpack_file_path(
    base_dir: &Path,
    file_name: &str,
) -> Result<PathBuf, LauncherErrorPayload> {
    let clean = launcher_security::validate_single_filename(file_name.trim())?;
    let resolved = launcher_security::safe_subpath(base_dir, clean)?;
    if resolved.parent() != Some(base_dir) || resolved.file_name().is_none() {
        return Err(LauncherErrorPayload::invalid_input(
            "Nama berkas modpack tidak valid.",
        ));
    }
    Ok(resolved)
}

fn check_modpack_cancelled(cancel_flag: &AtomicBool) -> Result<(), LauncherErrorPayload> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err(LauncherErrorPayload::new(
            ErrorCode::Cancelled,
            "Instalasi modpack dibatalkan oleh pengguna.",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn install_modpack<F>(
    paths: &DataPaths,
    http: &HttpClient,
    downloader: &DownloadManager,
    instances: &InstancesStore,
    provider: ContentProviderId,
    version_id: &str,
    instance_name: Option<&str>,
    cf_api_key: &str,
    on_progress: F,
) -> Result<Instance, LauncherErrorPayload>
where
    F: Fn(&str, u32, u32) + Send + Sync + Clone + 'static,
{
    install_modpack_with_cancel(
        paths,
        http,
        downloader,
        instances,
        provider,
        version_id,
        instance_name,
        cf_api_key,
        on_progress,
        Arc::new(AtomicBool::new(false)),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn install_modpack_with_cancel<F>(
    paths: &DataPaths,
    http: &HttpClient,
    downloader: &DownloadManager,
    instances: &InstancesStore,
    provider: ContentProviderId,
    version_id: &str,
    instance_name: Option<&str>,
    cf_api_key: &str,
    on_progress: F,
    cancel_flag: Arc<AtomicBool>,
) -> Result<Instance, LauncherErrorPayload>
where
    F: Fn(&str, u32, u32) + Send + Sync + Clone + 'static,
{
    install_modpack_with_cancel_and_options(
        paths,
        http,
        downloader,
        instances,
        provider,
        version_id,
        instance_name,
        cf_api_key,
        CF_API_BASE,
        ModpackInstallOptions::default(),
        on_progress,
        cancel_flag,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn install_modpack_with_cancel_and_options<F>(
    paths: &DataPaths,
    http: &HttpClient,
    downloader: &DownloadManager,
    instances: &InstancesStore,
    provider: ContentProviderId,
    version_id: &str,
    instance_name: Option<&str>,
    cf_api_key: &str,
    curseforge_base: &str,
    options: ModpackInstallOptions,
    on_progress: F,
    cancel_flag: Arc<AtomicBool>,
) -> Result<Instance, LauncherErrorPayload>
where
    F: Fn(&str, u32, u32) + Send + Sync + Clone + 'static,
{
    install_modpack_with_bases_and_options(
        paths,
        http,
        downloader,
        instances,
        provider,
        version_id,
        instance_name,
        cf_api_key,
        MR_API_BASE,
        curseforge_base,
        options,
        on_progress,
        cancel_flag,
    )
    .await
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn install_modpack_with_base<F>(
    paths: &DataPaths,
    http: &HttpClient,
    downloader: &DownloadManager,
    instances: &InstancesStore,
    provider: ContentProviderId,
    version_id: &str,
    instance_name: Option<&str>,
    cf_api_key: &str,
    curseforge_base: &str,
    on_progress: F,
) -> Result<Instance, LauncherErrorPayload>
where
    F: Fn(&str, u32, u32) + Send + Sync + Clone + 'static,
{
    install_modpack_with_bases_and_options(
        paths,
        http,
        downloader,
        instances,
        provider,
        version_id,
        instance_name,
        cf_api_key,
        MR_API_BASE,
        curseforge_base,
        ModpackInstallOptions::default(),
        on_progress,
        Arc::new(AtomicBool::new(false)),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn install_modpack_with_bases_and_options<F>(
    paths: &DataPaths,
    http: &HttpClient,
    downloader: &DownloadManager,
    instances: &InstancesStore,
    provider: ContentProviderId,
    version_id: &str,
    instance_name: Option<&str>,
    cf_api_key: &str,
    modrinth_base: &str,
    curseforge_base: &str,
    options: ModpackInstallOptions,
    on_progress: F,
    cancel_flag: Arc<AtomicBool>,
) -> Result<Instance, LauncherErrorPayload>
where
    F: Fn(&str, u32, u32) + Send + Sync + Clone + 'static,
{
    check_modpack_cancelled(&cancel_flag)?;
    on_progress("Menyiapkan folder unduhan modpack...", 1, 3);
    let staging_id = uuid::Uuid::new_v4().to_string();
    let staging_dir = paths.cache_dir.join("modpack-staging").join(&staging_id);
    fs::create_dir_all(&staging_dir).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal membuat folder staging: {}", e))
    })?;

    struct StagingDirGuard {
        root: PathBuf,
        dir: PathBuf,
    }
    impl Drop for StagingDirGuard {
        fn drop(&mut self) {
            let _ = launcher_security::safe_remove_dir_all_within(&self.root, &self.dir);
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
    let _staging_guard = StagingDirGuard {
        root: paths.root.clone(),
        dir: staging_dir.clone(),
    };

    let meta_res: Result<(String, Option<String>, ResolvedModpackMeta), LauncherErrorPayload> =
        async {
            check_modpack_cancelled(&cancel_flag)?;
            let provided_icon = sanitize_icon_url(options.icon_url.as_deref());
            match provider {
                ContentProviderId::Modrinth => {
                    let detail =
                        get_modrinth_version_with_base(http, modrinth_base, version_id).await?;
                    let cv = mr_to_version(&detail)?;
                    let url = cv.download_url.ok_or_else(|| {
                        LauncherErrorPayload::invalid_input(
                            "Modpack Modrinth tidak memiliki URL unduhan.",
                        )
                    })?;
                    let project_id = options
                        .project_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .or_else(|| {
                            let pid = detail.project_id.trim();
                            (!pid.is_empty()).then(|| pid.to_string())
                        });
                    let icon_url = match provided_icon {
                        Some(url) => Some(url),
                        None => match project_id.as_deref() {
                            Some(pid) => get_modrinth_project_with_base(http, modrinth_base, pid)
                                .await
                                .ok()
                                .and_then(|proj| {
                                    sanitize_icon_url(proj.summary.icon_url.as_deref())
                                }),
                            None => None,
                        },
                    };
                    let meta = ResolvedModpackMeta {
                        project_id,
                        version_id: detail.id,
                        file_name: cv.file_name,
                        sha1: cv.sha1.clone().unwrap_or_default(),
                        icon_url,
                    };
                    Ok((url, cv.sha1, meta))
                }
                ContentProviderId::Curseforge => {
                    let (m_str, f_str) = version_id.split_once(':').ok_or_else(|| {
                        LauncherErrorPayload::invalid_input(
                            "Format versionId CurseForge harus modId:fileId",
                        )
                    })?;
                    let m_id = m_str
                        .parse::<u64>()
                        .map_err(|_| LauncherErrorPayload::invalid_input("Invalid modId"))?;
                    let f_id = f_str
                        .parse::<u64>()
                        .map_err(|_| LauncherErrorPayload::invalid_input("Invalid fileId"))?;
                    let file = get_curseforge_file_with_base(
                        http,
                        curseforge_base,
                        cf_api_key,
                        &m_id.to_string(),
                        f_id,
                    )
                    .await?;
                    let cv = cf_to_version(&file);
                    let url = cv.download_url.ok_or_else(|| {
                        LauncherErrorPayload::distribution_blocked(
                            "Modpack CurseForge melarang distribusi langsung pihak ketiga.",
                        )
                    })?;
                    let project_id = options
                        .project_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .or_else(|| Some(m_id.to_string()));
                    let icon_url = match provided_icon {
                        Some(url) => Some(url),
                        None => get_curseforge_project_with_base(
                            http,
                            curseforge_base,
                            cf_api_key,
                            &m_id.to_string(),
                        )
                        .await
                        .ok()
                        .and_then(|proj| sanitize_icon_url(proj.summary.icon_url.as_deref())),
                    };
                    let meta = ResolvedModpackMeta {
                        project_id,
                        version_id: format!("{}:{}", m_id, f_id),
                        file_name: cv.file_name,
                        sha1: cv.sha1.clone().unwrap_or_default(),
                        icon_url,
                    };
                    Ok((url, cv.sha1, meta))
                }
            }
        }
        .await;

    let (download_url, sha1, modpack_meta) = meta_res.inspect_err(|_| {
        let _ = fs::remove_dir_all(&staging_dir);
    })?;

    let modpack_archive_path = safe_modpack_file_path(&staging_dir, &modpack_meta.file_name)
        .inspect_err(|_| {
            let _ = fs::remove_dir_all(&staging_dir);
        })?;

    on_progress("Mengunduh arsip modpack...", 1, 3);
    let on_p_arch = on_progress.clone();
    if let Err(e) = downloader
        .download_all(
            vec![DownloadTask {
                url: download_url,
                destination: modpack_archive_path.clone(),
                expected_sha1: sha1,
                expected_sha256: None,
                expected_size: None,
            }],
            move |p| {
                let mb = p.completed_bytes as f64 / (1024.0 * 1024.0);
                let msg = if p.total_bytes > 0 {
                    let total_mb = p.total_bytes as f64 / (1024.0 * 1024.0);
                    format!(
                        "Mengunduh arsip modpack ({:.1} / {:.1} MB · {})",
                        mb, total_mb, p.current_file
                    )
                } else if p.completed_bytes > 0 {
                    format!(
                        "Mengunduh arsip modpack ({:.1} MB · {})",
                        mb, p.current_file
                    )
                } else {
                    "Mengunduh arsip modpack...".to_string()
                };
                on_p_arch(&msg, 1, 3);
            },
            Arc::clone(&cancel_flag),
        )
        .await
    {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(if cancel_flag.load(Ordering::Relaxed) {
            LauncherErrorPayload::new(
                ErrorCode::Cancelled,
                "Instalasi modpack dibatalkan oleh pengguna.",
            )
        } else {
            e
        });
    }

    if let Err(e) = check_modpack_cancelled(&cancel_flag) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(e);
    }

    let modrinth_text = read_zip_text(&modpack_archive_path, "modrinth.index.json")?;
    let curseforge_text = read_zip_text(&modpack_archive_path, "manifest.json")?;

    if modrinth_text.is_none() && curseforge_text.is_none() {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(LauncherErrorPayload::invalid_input(
            "Berkas bukan arsip modpack yang valid (tidak ditemukan modrinth.index.json atau manifest.json).",
        ));
    }

    on_progress("Mengekstrak dan memproses struktur modpack...", 1, 3);
    let unpacked_dir = staging_dir.join("unpacked");
    extract_zip(&modpack_archive_path, &unpacked_dir, &[])?;

    let (
        mut instance,
        mut instance_guard,
        modpack_entry,
        download_items,
        content_entries,
        overrides_dir,
    ) = if let Some(m_json) = modrinth_text {
        let index: ModrinthIndex = serde_json::from_str(&m_json).map_err(|e| {
            LauncherErrorPayload::invalid_input(format!("Format modrinth.index.json rusak: {}", e))
        })?;

        let name = instance_name.unwrap_or(&index.name).trim();
        let minecraft_version = index
            .dependencies
            .get("minecraft")
            .cloned()
            .ok_or_else(|| {
                LauncherErrorPayload::invalid_input("Versi Minecraft tidak ditemukan pada modpack.")
            })?;
        let loader = parse_loader_from_modrinth(&index.dependencies);

        let (new_inst, instance_guard, modpack_entry) = create_pending_modpack_instance(
            paths,
            instances,
            name,
            minecraft_version,
            loader,
            provider,
            &modpack_meta,
        )?;

        let mc_dir = paths.instances_dir.join(&new_inst.id).join("minecraft");
        let mut items = Vec::new();
        let mut entries = Vec::new();

        for f in index.files {
            if let Some(url) = f.downloads.first() {
                if !is_allowed_mrpack_download_url(url) {
                    return Err(LauncherErrorPayload::invalid_input(format!(
                        "Domain URL unduhan modpack tidak diizinkan untuk '{}'",
                        f.path
                    )));
                }
                let sha1 = f.hashes.get("sha1").map(|s| s.trim().to_string());
                if !is_valid_sha1_hex(sha1.as_deref()) {
                    return Err(LauncherErrorPayload::invalid_input(format!(
                        "Checksum SHA-1 wajib tersedia pada file modpack '{}'",
                        f.path
                    )));
                }
                let dest = launcher_security::safe_subpath(&mc_dir, &f.path).map_err(|_| {
                    LauncherErrorPayload::invalid_input(format!(
                        "Path berkas berbahaya dalam modpack terdeteksi: '{}'",
                        f.path
                    ))
                })?;
                items.push(DownloadTask {
                    url: url.clone(),
                    destination: dest,
                    expected_sha1: sha1.clone(),
                    expected_sha256: None,
                    expected_size: f.file_size,
                });

                let norm_path = f.path.replace('\\', "/");
                let content_type = if norm_path.starts_with("mods/") {
                    Some(ContentType::Mod)
                } else if norm_path.starts_with("resourcepacks/") {
                    Some(ContentType::Resourcepack)
                } else if norm_path.starts_with("shaderpacks/") {
                    Some(ContentType::Shader)
                } else {
                    None
                };
                if let Some(content_type) = content_type {
                    entries.push(InstanceContentEntry {
                        provider: ContentProviderId::Modrinth,
                        project_id: f.path.clone(),
                        version_id: f.path.clone(),
                        content_type,
                        file_name: Path::new(&f.path)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("file")
                            .to_string(),
                        sha1: sha1.unwrap_or_default(),
                        world_folder: None,
                        disabled: Some(false),
                    });
                }
            }
        }

        (
            new_inst,
            instance_guard,
            modpack_entry,
            items,
            entries,
            unpacked_dir.join("overrides"),
        )
    } else {
        let c_json = curseforge_text.ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Manifest CurseForge tidak ditemukan")
        })?;
        let manifest: CurseForgeManifest = serde_json::from_str(&c_json).map_err(|e| {
            LauncherErrorPayload::invalid_input(format!("Format manifest.json rusak: {}", e))
        })?;

        let name = instance_name.unwrap_or(&manifest.name).trim();
        let minecraft_version = manifest.minecraft.version.clone();
        let loader = manifest
            .minecraft
            .mod_loaders
            .iter()
            .find(|l| l.primary.unwrap_or(false))
            .or_else(|| manifest.minecraft.mod_loaders.first())
            .map(|l| parse_loader_from_curseforge(&l.id))
            .unwrap_or_else(|| LoaderSpec {
                loader_type: LoaderType::Vanilla,
                version: None,
            });

        let (new_inst, instance_guard, modpack_entry) = create_pending_modpack_instance(
            paths,
            instances,
            name,
            minecraft_version,
            loader,
            provider,
            &modpack_meta,
        )?;

        let mc_dir = paths.instances_dir.join(&new_inst.id).join("minecraft");
        let mods_dir = mc_dir.join("mods");
        let mut items = Vec::new();
        let mut entries = Vec::new();
        let mut blocked_mods = Vec::new();
        let mut failed_mods = Vec::new();

        for file_ref in manifest.files {
            check_modpack_cancelled(&cancel_flag)?;
            match get_curseforge_file_with_base(
                http,
                curseforge_base,
                cf_api_key,
                &file_ref.project_id.to_string(),
                file_ref.file_id,
            )
            .await
            {
                Ok(file_detail) => {
                    let cv = cf_to_version(&file_detail);
                    let dl_url = match (file_detail.download_url.as_ref(), cv.download_url) {
                        (Some(_), Some(url)) => url,
                        _ => {
                            blocked_mods.push(format!(
                                "{} ({}:{})",
                                cv.file_name, file_ref.project_id, file_ref.file_id
                            ));
                            continue;
                        }
                    };
                    let dest = safe_modpack_file_path(&mods_dir, &cv.file_name)?;
                    items.push(DownloadTask {
                        url: dl_url,
                        destination: dest,
                        expected_sha1: cv.sha1.clone(),
                        expected_sha256: None,
                        expected_size: None,
                    });
                    entries.push(InstanceContentEntry {
                        provider: ContentProviderId::Curseforge,
                        project_id: file_ref.project_id.to_string(),
                        version_id: format!("{}:{}", file_ref.project_id, file_ref.file_id),
                        content_type: ContentType::Mod,
                        file_name: cv.file_name,
                        sha1: cv.sha1.unwrap_or_default(),
                        world_folder: None,
                        disabled: Some(false),
                    });
                }
                Err(e) => {
                    failed_mods.push(format!(
                        "{}:{} ({})",
                        file_ref.project_id, file_ref.file_id, e.message
                    ));
                }
            }
        }

        if !blocked_mods.is_empty() || !failed_mods.is_empty() {
            if !blocked_mods.is_empty() {
                let mut msg = format!(
                    "Modpack mengandung mod yang diblokir distribusinya oleh CurseForge: {}",
                    blocked_mods.join(", ")
                );
                if !failed_mods.is_empty() {
                    msg.push_str(&format!(
                        "; gagal mengambil metadata: {}",
                        failed_mods.join(", ")
                    ));
                }
                return Err(LauncherErrorPayload::distribution_blocked(msg));
            }
            return Err(LauncherErrorPayload::network(format!(
                "Gagal mengambil metadata mod CurseForge dalam modpack: {}",
                failed_mods.join(", ")
            )));
        }

        let overrides_name = manifest.overrides.trim();
        let overrides = if overrides_name.is_empty() {
            unpacked_dir.join("overrides")
        } else {
            launcher_security::safe_subpath(&unpacked_dir, overrides_name).map_err(|e| {
                LauncherErrorPayload::invalid_input(format!(
                    "Path folder overrides tidak valid (SEC-01): {}",
                    e.message
                ))
            })?
        };
        (
            new_inst,
            instance_guard,
            modpack_entry,
            items,
            entries,
            overrides,
        )
    };

    // Emit stage 2/3 only after the pending instance and iconUrl are persisted to disk.
    on_progress("Menyiapkan berkas modpack...", 2, 3);

    let mc_dir =
        launcher_security::safe_subpath(&paths.instances_dir, &instance.id)?.join("minecraft");

    let copy_res = if overrides_dir.exists() && overrides_dir.is_dir() {
        launcher_security::ensure_dir_within(&paths.instances_dir, &mc_dir, false).and_then(|_| {
            copy_recursive(&overrides_dir, &mc_dir).map_err(|e| {
                LauncherErrorPayload::file_system(format!("Gagal menyalin folder overrides: {}", e))
            })
        })
    } else {
        Ok(())
    };

    if let Err(e) = copy_res {
        let _ = instances.remove(&instance.id);
        let _ = launcher_security::safe_remove_dir_all_within(
            &paths.instances_dir,
            &paths.instances_dir.join(&instance.id),
        );
        let _ = launcher_security::safe_remove_dir_all_within(&paths.root, &staging_dir);
        return Err(e);
    }

    if !download_items.is_empty() {
        let total_mods = download_items.len() as u32;
        let on_p_mods = on_progress.clone();
        on_progress("Mengunduh berkas mod...", 2, 3);

        if let Err(e) = downloader
            .download_all(
                download_items,
                move |p| {
                    let mb = p.completed_bytes as f64 / (1024.0 * 1024.0);
                    let msg = if p.total_files > 0 {
                        let size_part = if p.total_bytes > 0 {
                            let total_mb = p.total_bytes as f64 / (1024.0 * 1024.0);
                            format!(" · {:.1}/{:.1} MB", mb, total_mb)
                        } else if p.completed_bytes > 0 {
                            format!(" · {:.1} MB", mb)
                        } else {
                            String::new()
                        };
                        format!(
                            "Mengunduh mod ({}/{}{} · {})",
                            p.completed_files, p.total_files, size_part, p.current_file
                        )
                    } else {
                        "Mengunduh mod...".to_string()
                    };
                    on_p_mods(&msg, p.completed_files as u32, total_mods.max(1));
                },
                Arc::clone(&cancel_flag),
            )
            .await
        {
            let _ = instances.remove(&instance.id);
            let _ = launcher_security::safe_remove_dir_all_within(
                &paths.instances_dir,
                &paths.instances_dir.join(&instance.id),
            );
            let _ = launcher_security::safe_remove_dir_all_within(&paths.root, &staging_dir);
            return Err(if cancel_flag.load(Ordering::Relaxed) {
                LauncherErrorPayload::new(
                    ErrorCode::Cancelled,
                    "Instalasi modpack dibatalkan oleh pengguna.",
                )
            } else {
                e
            });
        }
    }

    on_progress("Menyelesaikan konfigurasi instance modpack...", 3, 3);

    let mut final_content = content_entries;
    if let Some(entry) = modpack_entry {
        final_content.insert(0, entry);
    }
    instance.icon_url = modpack_meta.icon_url;
    instance.content = final_content;
    instances.update(instance.clone())?;
    let marker_path = paths
        .instances_dir
        .join(&instance.id)
        .join(launcher_storage::MODPACK_INSTALLING_MARKER);
    if marker_path.exists() {
        let _ = fs::remove_file(&marker_path);
    }
    instance_guard.disarm();

    Ok(instance)
}
