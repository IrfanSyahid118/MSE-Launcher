use sha1::{Digest, Sha1};
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use launcher_net::HttpClient;
use launcher_platform::DataPaths;
use launcher_storage::{read_json_file, write_json_atomic};
use launcher_types::{ErrorCode, LauncherErrorPayload, LoaderType};
use launcher_version::{MojangVersionDetail, VersionService};

use crate::fabric::{
    fetch_fabric_game_versions, fetch_fabric_loader_versions, fetch_fabric_profile,
};
use crate::forge::{fetch_forge_index, forge_versions_for_game};
use crate::merge::merge_version_detail;
use crate::models::{LoaderGameSupport, LoaderProfile, LoaderVersionInfo};
use crate::neoforge::{fetch_neoforge_index, neoforge_versions_for_game};
use crate::quilt::{fetch_quilt_game_versions, fetch_quilt_loader_versions, fetch_quilt_profile};

const CACHE_TTL: Duration = Duration::from_secs(10 * 60);

struct SupportCache {
    support: LoaderGameSupport,
    fetched_at: Instant,
}

pub enum InstallerPlan {
    /// Modern Forge 1.13+ / NeoForge: `install_profile.json` berisi `processors` yang perlu dieksekusi.
    ModernProcessors {
        installer_version_id: Option<String>,
        fallback_profile: Option<LoaderProfile>,
        jar_bytes: Vec<u8>,
    },
    /// Legacy Forge 1.8.9–1.11.2: `install_profile.json` berisi `versionInfo` + `install.filePath`.
    LegacyVersionInfo {
        profile: LoaderProfile,
        install_path: Option<String>,
        universal_jar_bytes: Option<Vec<u8>>,
    },
    /// Installer dengan `version.json` tanpa `processors`.
    SimpleVersionJson(LoaderProfile),
}

pub type InstallerRunner<'a> = Box<
    dyn Fn(&Path, &Path) -> Pin<Box<dyn Future<Output = Result<(), LauncherErrorPayload>> + Send>>
        + Send
        + Sync
        + 'a,
>;

fn is_safe_single_component(segment: &str) -> bool {
    launcher_security::validate_safe_single_component(segment).is_ok()
}

fn maven_coord_to_rel_path(coord: &str) -> Result<String, LauncherErrorPayload> {
    let (base, ext) = match coord.split_once('@') {
        Some((b, e)) => (b, e),
        None => (coord, "jar"),
    };
    launcher_security::validate_path_segment(ext)?;
    let parts: Vec<&str> = base.split(':').collect();
    if parts.len() < 3 {
        return Err(LauncherErrorPayload::invalid_input(
            "Koordinat Maven tidak valid.",
        ));
    }
    let mut group_segs = Vec::new();
    for seg in parts[0].split('.') {
        if seg.is_empty() {
            return Err(LauncherErrorPayload::invalid_input(
                "Group koordinat Maven tidak valid.",
            ));
        }
        launcher_security::validate_path_segment(seg)?;
        group_segs.push(seg);
    }
    let group_path = group_segs.join("/");
    let artifact = parts[1];
    let version = parts[2];
    launcher_security::validate_path_segment(artifact)?;
    launcher_security::validate_path_segment(version)?;
    let classifier = parts.get(3).copied().unwrap_or("");
    let filename = if classifier.is_empty() {
        format!("{artifact}-{version}.{ext}")
    } else {
        launcher_security::validate_path_segment(classifier)?;
        format!("{artifact}-{version}-{classifier}.{ext}")
    };
    launcher_security::validate_path_segment(&filename)?;
    let rel = format!("{group_path}/{artifact}/{version}/{filename}");
    launcher_security::SafeRelPath::parse(&rel)?;
    Ok(rel)
}

pub fn parse_installer_archive(bytes: &[u8]) -> Result<InstallerPlan, LauncherErrorPayload> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| LauncherErrorPayload::invalid_input(format!("Installer JAR rusak: {}", e)))?;

    use std::io::Read;
    let mut version_json_text: Option<String> = None;
    let mut install_profile_text: Option<String> = None;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal membaca entri JAR: {}", e))
        })?;
        let entry_name = match entry.name() {
            Ok(name) => name.to_string(),
            Err(_) => continue,
        };
        match entry_name.as_str() {
            "version.json" => {
                let mut text = String::new();
                entry.read_to_string(&mut text).map_err(|e| {
                    LauncherErrorPayload::file_system(format!("Gagal membaca version.json: {}", e))
                })?;
                version_json_text = Some(text);
            }
            "install_profile.json" => {
                let mut text = String::new();
                entry.read_to_string(&mut text).map_err(|e| {
                    LauncherErrorPayload::file_system(format!(
                        "Gagal membaca install_profile.json: {}",
                        e
                    ))
                })?;
                install_profile_text = Some(text);
            }
            _ => {}
        }
    }

    let fallback_profile = if let Some(ref raw) = version_json_text {
        Some(serde_json::from_str::<LoaderProfile>(raw).map_err(|e| {
            LauncherErrorPayload::invalid_input(format!(
                "Format version.json dalam installer tidak valid: {}",
                e
            ))
        })?)
    } else {
        None
    };

    if let Some(ref ip_raw) = install_profile_text {
        let ip_val: serde_json::Value = serde_json::from_str(ip_raw).map_err(|e| {
            LauncherErrorPayload::invalid_input(format!(
                "Format install_profile.json dalam installer tidak valid: {}",
                e
            ))
        })?;

        let has_processors = ip_val
            .get("processors")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| !arr.is_empty());

        if has_processors {
            let installer_version_id = ip_val
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            return Ok(InstallerPlan::ModernProcessors {
                installer_version_id,
                fallback_profile,
                jar_bytes: bytes.to_vec(),
            });
        }

        if let Some(version_info_val) = ip_val.get("versionInfo") {
            let mut profile: LoaderProfile = serde_json::from_value(version_info_val.clone())
                .map_err(|e| {
                    LauncherErrorPayload::invalid_input(format!(
                        "Format versionInfo dalam install_profile.json tidak valid: {}",
                        e
                    ))
                })?;

            if profile.inherits_from.trim().is_empty() {
                if let Some(mc) = ip_val
                    .get("install")
                    .and_then(|i| i.get("minecraft"))
                    .and_then(|m| m.as_str())
                {
                    profile.inherits_from = mc.to_string();
                }
            }

            let install_path = ip_val
                .get("install")
                .and_then(|i| i.get("path"))
                .and_then(|p| p.as_str())
                .map(|s| s.to_string());

            let install_file_path = ip_val
                .get("install")
                .and_then(|i| i.get("filePath"))
                .and_then(|f| f.as_str())
                .map(|s| s.to_string());

            let mut universal_jar_bytes = None;
            let maven_candidate = install_path
                .as_deref()
                .and_then(|p| maven_coord_to_rel_path(p).ok())
                .map(|rel| format!("maven/{rel}"));

            for candidate in [install_file_path.as_deref(), maven_candidate.as_deref()]
                .into_iter()
                .flatten()
            {
                if let Ok(mut entry) = archive.by_name(candidate) {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf).map_err(|e| {
                        LauncherErrorPayload::file_system(format!(
                            "Gagal mengekstrak JAR universal installer: {}",
                            e
                        ))
                    })?;
                    universal_jar_bytes = Some(buf);
                    break;
                }
            }

            return Ok(InstallerPlan::LegacyVersionInfo {
                profile,
                install_path,
                universal_jar_bytes,
            });
        }
    }

    if let Some(profile) = fallback_profile {
        return Ok(InstallerPlan::SimpleVersionJson(profile));
    }

    Err(LauncherErrorPayload::not_found(
        "version.json maupun install_profile.json tidak ditemukan dalam installer JAR.",
    ))
}

pub async fn fetch_installer_profile(
    http: &HttpClient,
    url: &str,
) -> Result<LoaderProfile, LauncherErrorPayload> {
    let resp = http.get(url).await?;
    let bytes = resp.bytes().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal mengunduh installer JAR: {}", e))
    })?;

    match parse_installer_archive(&bytes)? {
        InstallerPlan::SimpleVersionJson(p) => Ok(p),
        InstallerPlan::LegacyVersionInfo { profile, .. } => Ok(profile),
        InstallerPlan::ModernProcessors {
            fallback_profile: Some(p),
            ..
        } => Ok(p),
        InstallerPlan::ModernProcessors { .. } => Err(LauncherErrorPayload::not_found(
            "version.json tidak ditemukan dalam installer JAR.",
        )),
    }
}

fn find_java_executable(paths: &DataPaths, preferred: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = preferred {
        if let Ok(validated) = launcher_security::validate_java_executable_path(p) {
            return Some(validated);
        }
    }

    let exe_name = if cfg!(windows) { "java.exe" } else { "java" };

    if let Ok(entries) = std::fs::read_dir(&paths.runtimes_dir) {
        for entry in entries.flatten() {
            let bin_java = entry.path().join("bin").join(exe_name);
            if let Ok(validated) = launcher_security::validate_java_executable_path(&bin_java) {
                return Some(validated);
            }
        }
    }

    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let candidate = PathBuf::from(java_home).join("bin").join(exe_name);
        if let Ok(validated) = launcher_security::validate_java_executable_path(&candidate) {
            return Some(validated);
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(exe_name);
            if let Ok(validated) = launcher_security::validate_java_executable_path(&candidate) {
                return Some(validated);
            }
        }
    }

    None
}

async fn run_java_install_client(
    java_exe: &Path,
    installer_jar: &Path,
    shared_dir: &Path,
) -> Result<(), LauncherErrorPayload> {
    let validated_java = launcher_security::validate_java_executable_path(java_exe)?;
    let mut cmd = tokio::process::Command::new(validated_java);
    cmd.env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH")
        .arg("-jar")
        .arg(installer_jar)
        .arg("--installClient")
        .arg(shared_dir);

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = tokio::time::timeout(Duration::from_secs(300), cmd.output())
        .await
        .map_err(|_| {
            LauncherErrorPayload::new(
                ErrorCode::LaunchFailed,
                "Installer Forge/NeoForge melebihi batas waktu eksekusi.",
            )
        })?
        .map_err(|e| {
            LauncherErrorPayload::new(
                ErrorCode::LaunchFailed,
                format!("Gagal menjalankan installer Forge/NeoForge: {}", e),
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        return Err(LauncherErrorPayload::new(
            ErrorCode::LaunchFailed,
            format!(
                "Installer Forge/NeoForge gagal (exit {:?}): {}",
                output.status.code(),
                detail
            ),
        ));
    }

    Ok(())
}

#[derive(Clone)]
pub struct LoaderService {
    paths: DataPaths,
    http: HttpClient,
    versions: VersionService,
    support_cache: Arc<RwLock<Option<SupportCache>>>,
}

impl LoaderService {
    pub fn new(paths: DataPaths, versions: VersionService) -> Self {
        Self {
            paths,
            http: HttpClient::new(),
            versions,
            support_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn get_game_support(&self) -> Result<LoaderGameSupport, LauncherErrorPayload> {
        {
            let cache = self.support_cache.read().await;
            if let Some(ref c) = *cache {
                if c.fetched_at.elapsed() < CACHE_TTL {
                    return Ok(c.support.clone());
                }
            }
        }

        let fabric_fut = fetch_fabric_game_versions(&self.http);
        let quilt_fut = fetch_quilt_game_versions(&self.http);
        let forge_fut = fetch_forge_index(&self.http);
        let neoforge_fut = fetch_neoforge_index(&self.http);

        let (fabric_res, quilt_res, forge_res, neoforge_res) =
            tokio::join!(fabric_fut, quilt_fut, forge_fut, neoforge_fut);

        let support = LoaderGameSupport {
            fabric: fabric_res.ok(),
            quilt: quilt_res.ok(),
            forge: forge_res.ok().map(|idx| idx.game_versions),
            neoforge: neoforge_res.ok().map(|idx| idx.game_versions),
        };

        {
            let mut cache = self.support_cache.write().await;
            *cache = Some(SupportCache {
                support: support.clone(),
                fetched_at: Instant::now(),
            });
        }

        Ok(support)
    }

    pub async fn list_versions(
        &self,
        loader_type: LoaderType,
        minecraft_version: &str,
    ) -> Result<Vec<LoaderVersionInfo>, LauncherErrorPayload> {
        match loader_type {
            LoaderType::Vanilla => Ok(Vec::new()),
            LoaderType::Fabric => fetch_fabric_loader_versions(&self.http, minecraft_version).await,
            LoaderType::Quilt => fetch_quilt_loader_versions(&self.http, minecraft_version).await,
            LoaderType::Forge => {
                let index = fetch_forge_index(&self.http).await?;
                Ok(forge_versions_for_game(&index, minecraft_version))
            }
            LoaderType::Neoforge => {
                let index = fetch_neoforge_index(&self.http).await?;
                Ok(neoforge_versions_for_game(&index, minecraft_version))
            }
        }
    }

    pub async fn ensure_profile(
        &self,
        loader_type: LoaderType,
        minecraft_version: &str,
        loader_version: Option<&str>,
    ) -> Result<MojangVersionDetail, LauncherErrorPayload> {
        self.ensure_profile_with_java(loader_type, minecraft_version, loader_version, None)
            .await
    }

    pub async fn ensure_profile_with_java(
        &self,
        loader_type: LoaderType,
        minecraft_version: &str,
        loader_version: Option<&str>,
        java_executable: Option<&Path>,
    ) -> Result<MojangVersionDetail, LauncherErrorPayload> {
        let parent = self.versions.get_version_detail(minecraft_version).await?;

        if loader_type == LoaderType::Vanilla {
            return Ok(parent);
        }

        let chosen_version = if let Some(v) = loader_version {
            v.to_string()
        } else {
            let available = self.list_versions(loader_type, minecraft_version).await?;
            available
                .into_iter()
                .find(|v| v.stable)
                .map(|v| v.version)
                .ok_or_else(|| {
                    LauncherErrorPayload::not_found(format!(
                        "Tidak ada versi {:?} yang tersedia untuk Minecraft {}.",
                        loader_type, minecraft_version
                    ))
                })?
        };

        launcher_security::validate_safe_single_component(minecraft_version)?;
        launcher_security::validate_safe_single_component(&chosen_version)?;

        match loader_type {
            LoaderType::Fabric => {
                let profile_id = format!("fabric-loader-{}-{}", chosen_version, minecraft_version);
                launcher_security::validate_safe_single_component(&profile_id)?;
                let profile_dir =
                    launcher_security::safe_subpath(&self.paths.versions_dir, &profile_id)?;
                let profile_file =
                    launcher_security::safe_subpath(&profile_dir, &format!("{profile_id}.json"))?;

                let profile = if profile_file.exists() {
                    match read_json_file::<LoaderProfile>(&profile_file)? {
                        Some(p) => p,
                        None => {
                            let fresh = fetch_fabric_profile(
                                &self.http,
                                minecraft_version,
                                &chosen_version,
                            )
                            .await?;
                            let _ = write_json_atomic(&profile_file, &fresh);
                            fresh
                        }
                    }
                } else {
                    let fresh =
                        fetch_fabric_profile(&self.http, minecraft_version, &chosen_version)
                            .await?;
                    let _ = write_json_atomic(&profile_file, &fresh);
                    fresh
                };

                merge_version_detail(&parent, &profile)
            }
            LoaderType::Quilt => {
                let profile_id = format!("quilt-loader-{}-{}", chosen_version, minecraft_version);
                launcher_security::validate_safe_single_component(&profile_id)?;
                let profile_dir =
                    launcher_security::safe_subpath(&self.paths.versions_dir, &profile_id)?;
                let profile_file =
                    launcher_security::safe_subpath(&profile_dir, &format!("{profile_id}.json"))?;

                let profile = if profile_file.exists() {
                    match read_json_file::<LoaderProfile>(&profile_file)? {
                        Some(p) => p,
                        None => {
                            let fresh =
                                fetch_quilt_profile(&self.http, minecraft_version, &chosen_version)
                                    .await?;
                            let _ = write_json_atomic(&profile_file, &fresh);
                            fresh
                        }
                    }
                } else {
                    let fresh =
                        fetch_quilt_profile(&self.http, minecraft_version, &chosen_version).await?;
                    let _ = write_json_atomic(&profile_file, &fresh);
                    fresh
                };

                merge_version_detail(&parent, &profile)
            }
            LoaderType::Forge | LoaderType::Neoforge => {
                let profile_id = format!(
                    "{}-{}-{}",
                    match loader_type {
                        LoaderType::Forge => "forge",
                        _ => "neoforge",
                    },
                    minecraft_version,
                    chosen_version
                );
                launcher_security::validate_safe_single_component(&profile_id)?;

                let profile_dir =
                    launcher_security::safe_subpath(&self.paths.versions_dir, &profile_id)?;
                let profile_file =
                    launcher_security::safe_subpath(&profile_dir, &format!("{profile_id}.json"))?;
                let marker_file = launcher_security::safe_subpath(&profile_dir, ".installer_done")?;

                let profile = if profile_file.exists() && marker_file.exists() {
                    match read_json_file::<LoaderProfile>(&profile_file)? {
                        Some(p) => p,
                        None => {
                            self.install_forge_or_neoforge_from_maven(
                                loader_type,
                                minecraft_version,
                                &chosen_version,
                                &profile_id,
                                &parent,
                                java_executable,
                            )
                            .await?
                        }
                    }
                } else {
                    self.install_forge_or_neoforge_from_maven(
                        loader_type,
                        minecraft_version,
                        &chosen_version,
                        &profile_id,
                        &parent,
                        java_executable,
                    )
                    .await?
                };

                merge_version_detail(&parent, &profile)
            }
            LoaderType::Vanilla => Ok(parent),
        }
    }

    async fn install_forge_or_neoforge_from_maven(
        &self,
        loader_type: LoaderType,
        minecraft_version: &str,
        chosen_version: &str,
        profile_id: &str,
        parent: &MojangVersionDetail,
        java_executable: Option<&Path>,
    ) -> Result<LoaderProfile, LauncherErrorPayload> {
        let url = match loader_type {
            LoaderType::Forge => format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{}-{}/forge-{}-{}-installer.jar",
                minecraft_version, chosen_version, minecraft_version, chosen_version
            ),
            _ => format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
                chosen_version, chosen_version
            ),
        };

        let resp = self.http.get(&url).await?;
        let bytes = resp.bytes().await.map_err(|e| {
            LauncherErrorPayload::network(format!("Gagal mengunduh installer JAR: {}", e))
        })?;

        let sha1_url = format!("{url}.sha1");
        let sha_resp = self.http.get(&sha1_url).await.map_err(|_| {
            LauncherErrorPayload::network(
                "Gagal mengunduh checksum SHA-1 installer Forge/NeoForge.",
            )
        })?;
        let sha_text = sha_resp.text().await.map_err(|e| {
            LauncherErrorPayload::network(format!(
                "Gagal membaca checksum SHA-1 installer Forge/NeoForge: {}",
                e
            ))
        })?;
        let expected = sha_text.split_whitespace().next().unwrap_or("").trim();
        if expected.len() != 40 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(LauncherErrorPayload::network(
                "Checksum SHA-1 installer Forge/NeoForge tidak valid atau kosong.",
            ));
        }
        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(LauncherErrorPayload::network(
                "Checksum SHA-1 installer Forge/NeoForge tidak cocok.",
            ));
        }

        let java_bin = java_executable.map(|p| p.to_path_buf());
        let http = self.http.clone();
        let runner: InstallerRunner<'_> = Box::new(move |jar_path, shared_dir| {
            let jar_path = jar_path.to_path_buf();
            let shared_dir = shared_dir.to_path_buf();
            let java_bin = java_bin.clone();
            Box::pin(async move {
                let paths = DataPaths::new(shared_dir.parent().unwrap_or(&shared_dir));
                let exe = find_java_executable(&paths, java_bin.as_deref()).ok_or_else(|| {
                    LauncherErrorPayload::new(
                        ErrorCode::JavaMissing,
                        "Java tidak ditemukan untuk menjalankan installer Forge/NeoForge.",
                    )
                })?;
                let _ = http;
                run_java_install_client(&exe, &jar_path, &shared_dir).await
            })
        });

        self.prepare_installer_from_bytes(
            &bytes,
            minecraft_version,
            profile_id,
            Some(parent),
            runner,
        )
        .await
    }

    pub async fn prepare_installer_from_bytes<'a>(
        &self,
        bytes: &[u8],
        minecraft_version: &str,
        profile_id: &str,
        parent: Option<&MojangVersionDetail>,
        runner: InstallerRunner<'a>,
    ) -> Result<LoaderProfile, LauncherErrorPayload> {
        launcher_security::validate_safe_single_component(minecraft_version)?;
        launcher_security::validate_safe_single_component(profile_id)?;
        let profile_dir = launcher_security::safe_subpath(&self.paths.versions_dir, profile_id)?;
        launcher_security::ensure_dir_within(&self.paths.versions_dir, &profile_dir, false)?;
        let profile_file =
            launcher_security::safe_subpath(&profile_dir, &format!("{profile_id}.json"))?;
        let marker_file = launcher_security::safe_subpath(&profile_dir, ".installer_done")?;

        let plan = parse_installer_archive(bytes)?;
        let resolved_profile = match plan {
            InstallerPlan::SimpleVersionJson(mut profile) => {
                if profile.inherits_from.trim().is_empty() {
                    profile.inherits_from = minecraft_version.to_string();
                }
                profile.id = profile_id.to_string();
                profile
            }
            InstallerPlan::LegacyVersionInfo {
                mut profile,
                install_path,
                universal_jar_bytes,
            } => {
                if profile.inherits_from.trim().is_empty() {
                    profile.inherits_from = minecraft_version.to_string();
                }
                profile.id = profile_id.to_string();

                if let (Some(coord), Some(jar_bytes)) = (install_path, universal_jar_bytes) {
                    let rel = maven_coord_to_rel_path(&coord)?;
                    let dest = launcher_security::safe_subpath(&self.paths.libraries_dir, &rel)?;
                    if let Some(parent_dir) = dest.parent() {
                        launcher_security::ensure_dir_within(
                            &self.paths.libraries_dir,
                            parent_dir,
                            false,
                        )?;
                    }
                    launcher_security::ensure_descendants_no_symlink(
                        &self.paths.libraries_dir,
                        &dest,
                    )?;
                    std::fs::write(&dest, jar_bytes).map_err(|e| {
                        LauncherErrorPayload::file_system(format!(
                            "Gagal menyimpan universal JAR legacy Forge: {}",
                            e
                        ))
                    })?;
                }

                profile
            }
            InstallerPlan::ModernProcessors {
                installer_version_id,
                fallback_profile,
                jar_bytes,
            } => {
                let parent_jar = launcher_security::safe_subpath(
                    &self.paths.versions_dir,
                    &format!("{minecraft_version}/{minecraft_version}.jar"),
                )?;
                if !parent_jar.exists() {
                    if let Some(p_detail) = parent {
                        if let Some(client_art) =
                            p_detail.downloads.as_ref().and_then(|d| d.client.as_ref())
                        {
                            if let Some(p_dir) = parent_jar.parent() {
                                launcher_security::ensure_dir_within(
                                    &self.paths.versions_dir,
                                    p_dir,
                                    false,
                                )?;
                            }
                            launcher_security::ensure_descendants_no_symlink(
                                &self.paths.versions_dir,
                                &parent_jar,
                            )?;
                            let resp = self.http.get(&client_art.url).await?;
                            let jar_data = resp.bytes().await.map_err(|e| {
                                LauncherErrorPayload::network(format!(
                                    "Gagal mengunduh vanilla client JAR: {}",
                                    e
                                ))
                            })?;
                            if client_art.size > 0 && jar_data.len() as u64 != client_art.size {
                                return Err(LauncherErrorPayload::network(
                                    "Ukuran vanilla client JAR tidak sesuai dengan manifes Mojang.",
                                ));
                            }
                            let expected_sha1 = client_art.sha1.trim();
                            if expected_sha1.len() != 40
                                || !expected_sha1.bytes().all(|b| b.is_ascii_hexdigit())
                            {
                                return Err(LauncherErrorPayload::network(
                                    "Checksum SHA-1 vanilla client JAR pada manifes tidak valid.",
                                ));
                            }
                            let mut hasher = Sha1::new();
                            hasher.update(&jar_data);
                            let actual_sha1 = hex::encode(hasher.finalize());
                            if !actual_sha1.eq_ignore_ascii_case(expected_sha1) {
                                return Err(LauncherErrorPayload::network(
                                    "Checksum SHA-1 vanilla client JAR tidak cocok.",
                                ));
                            }
                            std::fs::write(&parent_jar, &jar_data).map_err(|e| {
                                LauncherErrorPayload::file_system(format!(
                                    "Gagal menyimpan vanilla client JAR: {}",
                                    e
                                ))
                            })?;
                        }
                    }
                }

                launcher_security::ensure_secure_dir(&self.paths.shared_dir, false)?;
                let launcher_profiles = launcher_security::safe_subpath(
                    &self.paths.shared_dir,
                    "launcher_profiles.json",
                )?;
                launcher_security::ensure_descendants_no_symlink(
                    &self.paths.shared_dir,
                    &launcher_profiles,
                )?;
                if !launcher_profiles.exists() {
                    std::fs::write(
                        &launcher_profiles,
                        br#"{"profiles":{},"clientToken":"","authenticationDatabase":{},"selectedProfile":""}"#,
                    )
                    .map_err(|e| {
                        LauncherErrorPayload::file_system(format!(
                            "Gagal membuat launcher_profiles.json stub: {}",
                            e
                        ))
                    })?;
                }

                launcher_security::ensure_secure_dir(&self.paths.cache_dir, true)?;
                let nonce = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let temp_jar = launcher_security::safe_subpath(
                    &self.paths.cache_dir,
                    &format!("loader-installer-{}-{}.jar", nonce, std::process::id()),
                )?;
                {
                    let mut temp_file = launcher_security::safe_create_new_file(&temp_jar, false)?;
                    temp_file.write_all(&jar_bytes).map_err(|e| {
                        LauncherErrorPayload::file_system(format!(
                            "Gagal menyimpan installer JAR sementara: {}",
                            e
                        ))
                    })?;
                }

                let run_res = runner(&temp_jar, &self.paths.shared_dir).await;
                let _ =
                    launcher_security::safe_remove_file_within(&self.paths.cache_dir, &temp_jar);
                run_res?;

                let mut generated: Option<LoaderProfile> = None;
                if let Some(ref inst_ver_id) = installer_version_id {
                    if is_safe_single_component(inst_ver_id) {
                        if let Ok(inst_json) = launcher_security::safe_subpath(
                            &self.paths.versions_dir,
                            &format!("{inst_ver_id}/{inst_ver_id}.json"),
                        ) {
                            if inst_json.exists()
                                && launcher_security::ensure_descendants_no_symlink(
                                    &self.paths.versions_dir,
                                    &inst_json,
                                )
                                .is_ok()
                            {
                                generated = read_json_file::<LoaderProfile>(&inst_json)?;
                            }
                        }
                    }
                }

                let mut profile = generated.or(fallback_profile).ok_or_else(|| {
                    LauncherErrorPayload::not_found(
                        "Profil versi hasil installer Forge/NeoForge tidak ditemukan.",
                    )
                })?;
                if profile.inherits_from.trim().is_empty() {
                    profile.inherits_from = minecraft_version.to_string();
                }
                profile.id = profile_id.to_string();
                profile
            }
        };

        let _ = write_json_atomic(&profile_file, &resolved_profile);
        let _ = launcher_security::ensure_dir_within(&self.paths.versions_dir, &profile_dir, false);
        let _ = std::fs::write(&marker_file, b"ok");
        Ok(resolved_profile)
    }
}

#[cfg(test)]
mod ps_tests {
    use super::maven_coord_to_rel_path;

    #[test]
    fn test_ps04_maven_coord_traversal_rejected() {
        assert!(maven_coord_to_rel_path("net.minecraftforge:forge:1.20.1-47.2.0").is_ok());
        assert!(maven_coord_to_rel_path("com.evil:../pwn:1.0").is_err());
        assert!(maven_coord_to_rel_path("com..evil:mod:1.0").is_err());
        assert!(maven_coord_to_rel_path("com.evil:mod:1.0@exe/evil").is_err());
        assert!(maven_coord_to_rel_path("com.evil:CON:1.0").is_err());
    }
}
