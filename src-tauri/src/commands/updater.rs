use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::state::AppState;
use launcher_download::DownloadTask;
use launcher_net::HttpClient;
use launcher_types::{ErrorCode, InstallMode, LauncherErrorPayload, UpdateCheckResult};
use serde::Deserialize;
use serde_json::json;
use tauri::Emitter;

const GITHUB_OWNER: &str = "IrfanSyahid118";
const GITHUB_REPO: &str = "MSE-Launcher";
const LATEST_MANIFEST_URL: &str =
    "https://github.com/IrfanSyahid118/MSE-Launcher/releases/latest/download/latest.json";
const RELEASES_API_URL: &str =
    "https://api.github.com/repos/IrfanSyahid118/MSE-Launcher/releases?per_page=5";

fn is_valid_sha256_hex(value: Option<&str>) -> bool {
    value
        .map(str::trim)
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LatestManifest {
    version: String,
    notes: Option<String>,
    platforms: Option<std::collections::HashMap<String, PlatformManifest>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlatformManifest {
    installer: Option<ManifestAsset>,
    portable: Option<ManifestAsset>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestAsset {
    name: String,
    url: String,
    sha256: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: Option<u64>,
}

pub fn detect_install_mode_for_path(exe_path: &Path) -> InstallMode {
    if let Some(name) = exe_path.file_name().and_then(|s| s.to_str()) {
        if name.to_ascii_lowercase().contains("portable") {
            return InstallMode::Portable;
        }
    }

    if let Some(exe_dir) = exe_path.parent() {
        if exe_dir.join("portable.txt").exists() || exe_dir.join(".portable").exists() {
            return InstallMode::Portable;
        }
        if exe_dir.join("uninstall.exe").exists() {
            return InstallMode::Installer;
        }
    }

    let normalized = exe_path
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    if normalized.contains("\\program files\\")
        || normalized.contains("\\program files (x86)\\")
        || normalized.contains("\\appdata\\local\\programs\\")
    {
        return InstallMode::Installer;
    }

    InstallMode::Portable
}

pub fn detect_current_install_mode() -> InstallMode {
    std::env::current_exe()
        .map(|p| detect_install_mode_for_path(&p))
        .unwrap_or(InstallMode::Portable)
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedVersion {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Option<String>,
}

fn parse_semver(raw: &str) -> Option<ParsedVersion> {
    let without_build = raw
        .trim()
        .trim_start_matches(['v', 'V'])
        .split('+')
        .next()?;
    let (core, pre) = match without_build.split_once('-') {
        Some((c, p)) => {
            let p = p.trim();
            (
                c,
                if p.is_empty() {
                    None
                } else {
                    Some(p.to_ascii_lowercase())
                },
            )
        }
        None => (without_build, None),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    Some(ParsedVersion {
        major,
        minor,
        patch,
        prerelease: pre,
    })
}

fn compare_prerelease(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut left_parts = left.split('.');
    let mut right_parts = right.split('.');
    loop {
        match (left_parts.next(), right_parts.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let ord = match (a.parse::<u64>(), b.parse::<u64>()) {
                    (Ok(na), Ok(nb)) => na.cmp(&nb),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => a.cmp(b),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let (Some(curr), Some(cand)) = (parse_semver(current), parse_semver(latest)) else {
        return false;
    };
    let curr_tuple = (curr.major, curr.minor, curr.patch);
    let cand_tuple = (cand.major, cand.minor, cand.patch);
    if cand_tuple != curr_tuple {
        return cand_tuple > curr_tuple;
    }
    match (curr.prerelease, cand.prerelease) {
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (Some(c_pre), Some(l_pre)) => compare_prerelease(&l_pre, &c_pre).is_gt(),
        (None, None) => false,
    }
}

pub fn validate_update_url(raw_url: &str) -> Result<url::Url, LauncherErrorPayload> {
    let parsed = url::Url::parse(raw_url.trim()).map_err(|e| {
        LauncherErrorPayload::invalid_input(format!("URL pembaruan tidak valid: {e}"))
    })?;
    if parsed.scheme() != "https" {
        return Err(LauncherErrorPayload::invalid_input(
            "URL pembaruan wajib menggunakan protokol HTTPS.",
        ));
    }
    let host = parsed
        .host_str()
        .map(|h| h.to_ascii_lowercase())
        .unwrap_or_default();

    let allowed = host == "github.com"
        && parsed.path().starts_with(&format!(
            "/{}/{}/releases/download/",
            GITHUB_OWNER, GITHUB_REPO
        ));

    if !allowed {
        return Err(LauncherErrorPayload::invalid_input(
            "Domain URL pembaruan tidak berasal dari repositori rilis resmi.",
        ));
    }

    Ok(parsed)
}

fn current_platform_key() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "macos-aarch64"
    } else if cfg!(target_os = "macos") {
        "macos-x86_64"
    } else {
        "linux-x86_64"
    }
}

fn validate_asset_filename(asset_name: &str) -> Result<String, LauncherErrorPayload> {
    let trimmed = asset_name.trim();
    launcher_security::validate_single_filename(trimmed)?;
    let lower = trimmed.to_ascii_lowercase();
    #[cfg(windows)]
    {
        if !lower.ends_with(".exe") {
            return Err(LauncherErrorPayload::invalid_input(
                "Berkas pembaruan Windows harus berekstensi .exe.",
            ));
        }
    }
    #[cfg(not(windows))]
    {
        let allowed = lower.ends_with(".appimage")
            || lower.ends_with(".deb")
            || lower.ends_with(".rpm")
            || lower.ends_with(".tar.gz")
            || lower.ends_with(".bin")
            || lower.ends_with(".exe")
            || !lower.contains('.');
        if !allowed {
            return Err(LauncherErrorPayload::invalid_input(
                "Ekstensi berkas pembaruan tidak didukung.",
            ));
        }
    }
    Ok(trimmed.to_string())
}

fn select_github_asset(assets: &[GitHubAsset], mode: InstallMode) -> Option<&GitHubAsset> {
    #[cfg(windows)]
    {
        match mode {
            InstallMode::Portable => assets.iter().find(|asset| {
                let lower = asset.name.to_ascii_lowercase();
                lower.contains("portable") && lower.ends_with(".exe")
            }),
            InstallMode::Installer => assets
                .iter()
                .find(|asset| asset.name.to_ascii_lowercase().ends_with("-setup.exe"))
                .or_else(|| {
                    assets.iter().find(|asset| {
                        let lower = asset.name.to_ascii_lowercase();
                        lower.ends_with(".exe") && !lower.contains("portable")
                    })
                }),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = mode;
        assets.iter().find(|asset| {
            let lower = asset.name.to_ascii_lowercase();
            lower.ends_with(".appimage")
                || lower.ends_with(".deb")
                || lower.ends_with(".rpm")
                || lower.ends_with(".tar.gz")
        })
    }
}

fn parse_manifest_result(
    manifest: LatestManifest,
    current_version: &str,
    install_mode: InstallMode,
) -> Option<UpdateCheckResult> {
    let platform = manifest.platforms.as_ref()?.get(current_platform_key())?;
    let asset = match install_mode {
        InstallMode::Portable => platform.portable.as_ref(),
        InstallMode::Installer => platform.installer.as_ref(),
    };

    let (asset_name, download_url, size_bytes, sha256) = if let Some(a) = asset {
        let valid_sha = a
            .sha256
            .as_deref()
            .map(str::trim)
            .filter(|s| is_valid_sha256_hex(Some(*s)))
            .map(|s| s.to_ascii_lowercase());
        if validate_update_url(&a.url).is_ok()
            && validate_asset_filename(&a.name).is_ok()
            && valid_sha.is_some()
        {
            (Some(a.name.clone()), Some(a.url.clone()), a.size, valid_sha)
        } else {
            (None, None, None, None)
        }
    } else {
        (None, None, None, None)
    };

    let latest_version = manifest.version.trim_start_matches(['v', 'V']).to_string();
    let update_available =
        is_newer_version(current_version, &latest_version) && download_url.is_some();

    Some(UpdateCheckResult {
        current_version: current_version.to_string(),
        latest_version,
        update_available,
        install_mode,
        asset_name,
        download_url,
        size_bytes,
        sha256,
        release_notes: manifest.notes,
    })
}

async fn try_fetch_manifest_from_url(
    http: &HttpClient,
    manifest_url: &str,
    current_version: &str,
    install_mode: InstallMode,
) -> Option<UpdateCheckResult> {
    let resp = http
        .request(reqwest::Method::GET, manifest_url)
        .ok()?
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let manifest: LatestManifest = resp.json().await.ok()?;
    parse_manifest_result(manifest, current_version, install_mode)
}

async fn try_fetch_latest_manifest(
    http: &HttpClient,
    current_version: &str,
    install_mode: InstallMode,
) -> Option<UpdateCheckResult> {
    try_fetch_manifest_from_url(http, LATEST_MANIFEST_URL, current_version, install_mode).await
}

fn select_release(releases: &[GitHubRelease]) -> Option<&GitHubRelease> {
    let non_draft: Vec<&GitHubRelease> = releases.iter().filter(|r| !r.draft).collect();
    let mut best: Option<&GitHubRelease> = None;
    for cand in non_draft {
        best = match best {
            None => Some(cand),
            Some(cur) => {
                let cand_ver = cand.tag_name.trim_start_matches(['v', 'V']);
                let cur_ver = cur.tag_name.trim_start_matches(['v', 'V']);
                if is_newer_version(cur_ver, cand_ver)
                    || (!is_newer_version(cand_ver, cur_ver) && cur.prerelease && !cand.prerelease)
                {
                    Some(cand)
                } else {
                    Some(cur)
                }
            }
        };
    }
    best
}

fn parse_sha256_from_checksum_text(text: &str, asset_name: &str) -> Option<String> {
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // BSD format: `SHA256 (filename) = <64-hex>`
        if let Some(rest) = line
            .strip_prefix("SHA256 (")
            .or_else(|| line.strip_prefix("sha256 ("))
        {
            if let Some((name, hash_part)) = rest.split_once(") = ") {
                let candidate = hash_part.trim().to_ascii_lowercase();
                if name.trim().eq_ignore_ascii_case(asset_name)
                    && is_valid_sha256_hex(Some(candidate.as_str()))
                {
                    return Some(candidate);
                }
            }
            continue;
        }
        // Single-hash file content (exact 64-hex line)
        let lower_line = line.to_ascii_lowercase();
        if is_valid_sha256_hex(Some(lower_line.as_str())) {
            return Some(lower_line);
        }
        // GNU sha256sum format: `<64-hex> [* ]<filename>`
        let mut parts = line.split_whitespace();
        if let (Some(hash_tok), Some(file_tok)) = (parts.next(), parts.next()) {
            let clean_hash = hash_tok.trim().to_ascii_lowercase();
            let clean_file = file_tok
                .trim_start_matches('*')
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(file_tok);
            if clean_file.eq_ignore_ascii_case(asset_name)
                && is_valid_sha256_hex(Some(clean_hash.as_str()))
            {
                return Some(clean_hash);
            }
        }
    }
    None
}

async fn try_fetch_companion_sha256(
    http: &HttpClient,
    release: &GitHubRelease,
    asset_name: &str,
) -> Option<String> {
    let direct_candidates = [
        format!("{asset_name}.sha256"),
        format!("{asset_name}.sha256sum"),
    ];
    let aggregate_candidates = ["sha256sums", "sha256sums.txt", "checksums.txt"];

    let checksum_asset = release
        .assets
        .iter()
        .find(|a| {
            direct_candidates
                .iter()
                .any(|cand| a.name.eq_ignore_ascii_case(cand))
        })
        .or_else(|| {
            release.assets.iter().find(|a| {
                aggregate_candidates
                    .iter()
                    .any(|cand| a.name.eq_ignore_ascii_case(cand))
            })
        })?;

    if checksum_asset.size.unwrap_or(0) > 64 * 1024 {
        return None;
    }
    validate_update_url(&checksum_asset.browser_download_url).ok()?;
    let resp = http
        .request(reqwest::Method::GET, &checksum_asset.browser_download_url)
        .ok()?
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().await.ok()?;
    if body.len() > 64 * 1024 {
        return None;
    }
    parse_sha256_from_checksum_text(&body, asset_name)
}

async fn build_release_fallback_result(
    http: &HttpClient,
    release: &GitHubRelease,
    current_version: &str,
    install_mode: InstallMode,
) -> UpdateCheckResult {
    let latest_version = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    let asset = select_github_asset(&release.assets, install_mode).filter(|&a| {
        validate_update_url(&a.browser_download_url).is_ok()
            && validate_asset_filename(&a.name).is_ok()
    });
    let download_url = asset.map(|a| a.browser_download_url.clone());
    let sha256 = match asset {
        Some(a) => try_fetch_companion_sha256(http, release, &a.name).await,
        None => None,
    };
    let update_available =
        is_newer_version(current_version, &latest_version) && download_url.is_some();

    UpdateCheckResult {
        current_version: current_version.to_string(),
        latest_version,
        update_available,
        install_mode,
        asset_name: asset.map(|a| a.name.clone()),
        download_url,
        size_bytes: asset.and_then(|a| a.size),
        sha256,
        release_notes: release.body.clone(),
    }
}

async fn try_fetch_release_manifest_asset(
    http: &HttpClient,
    release: &GitHubRelease,
    current_version: &str,
    install_mode: InstallMode,
) -> Option<UpdateCheckResult> {
    let manifest_asset = release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case("latest.json"))?;
    validate_update_url(&manifest_asset.browser_download_url).ok()?;
    try_fetch_manifest_from_url(
        http,
        &manifest_asset.browser_download_url,
        current_version,
        install_mode,
    )
    .await
}

async fn try_fetch_any_verified_manifest(
    http: &HttpClient,
    current_version: &str,
    install_mode: InstallMode,
) -> Option<UpdateCheckResult> {
    if let Some(latest) = try_fetch_latest_manifest(http, current_version, install_mode).await {
        if latest.update_available && latest.download_url.is_some() {
            return Some(latest);
        }
    }
    let resp = http
        .request(reqwest::Method::GET, RELEASES_API_URL)
        .ok()?
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let releases: Vec<GitHubRelease> = resp.json().await.ok()?;
    let release = select_release(&releases)?;
    if let Some(manifest) =
        try_fetch_release_manifest_asset(http, release, current_version, install_mode).await
    {
        return Some(manifest);
    }
    let fallback =
        build_release_fallback_result(http, release, current_version, install_mode).await;
    fallback
        .sha256
        .as_deref()
        .filter(|s| is_valid_sha256_hex(Some(*s)))?;
    Some(fallback)
}

#[tauri::command]
pub async fn updater_check(
    state: tauri::State<'_, AppState>,
) -> Result<UpdateCheckResult, LauncherErrorPayload> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let install_mode = detect_current_install_mode();
    let http = HttpClient::new();

    let latest_manifest = try_fetch_latest_manifest(&http, &current_version, install_mode).await;
    if let Some(ref manifest_res) = latest_manifest {
        if manifest_res.update_available && manifest_res.download_url.is_some() {
            *state.verified_update.write().await = Some(manifest_res.clone());
            return Ok(manifest_res.clone());
        }
    }

    *state.verified_update.write().await = None;

    let resp = http
        .request(reqwest::Method::GET, RELEASES_API_URL)?
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| {
            LauncherErrorPayload::network(format!("Gagal memeriksa pembaruan dari GitHub: {e}"))
        })?;

    let status = resp.status();
    if status.as_u16() == 404 {
        if let Some(manifest_res) = latest_manifest {
            *state.verified_update.write().await = Some(manifest_res.clone());
            return Ok(manifest_res);
        }
        return Ok(UpdateCheckResult {
            current_version: current_version.clone(),
            latest_version: current_version,
            update_available: false,
            install_mode,
            asset_name: None,
            download_url: None,
            size_bytes: None,
            sha256: None,
            release_notes: None,
        });
    }

    if !status.is_success() {
        return Err(LauncherErrorPayload::network(format!(
            "Pemeriksaan pembaruan GitHub gagal dengan status HTTP {status}."
        )));
    }

    let releases: Vec<GitHubRelease> = resp.json().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal membaca respons rilis GitHub: {e}"))
    })?;

    let Some(release) = select_release(&releases) else {
        if let Some(manifest_res) = latest_manifest {
            *state.verified_update.write().await = Some(manifest_res.clone());
            return Ok(manifest_res);
        }
        return Ok(UpdateCheckResult {
            current_version: current_version.clone(),
            latest_version: current_version,
            update_available: false,
            install_mode,
            asset_name: None,
            download_url: None,
            size_bytes: None,
            sha256: None,
            release_notes: None,
        });
    };

    if let Some(mut manifest_res) =
        try_fetch_release_manifest_asset(&http, release, &current_version, install_mode).await
    {
        if manifest_res.release_notes.is_none() {
            manifest_res.release_notes = release.body.clone();
        }
        *state.verified_update.write().await = Some(manifest_res.clone());
        return Ok(manifest_res);
    }

    let result =
        build_release_fallback_result(&http, release, &current_version, install_mode).await;
    if result.sha256.is_some() {
        *state.verified_update.write().await = Some(result.clone());
    }
    Ok(result)
}

fn portable_staging_destination(
    current_exe: &Path,
    paths: &launcher_platform::DataPaths,
    asset_name: &str,
    staging_id: &str,
) -> (PathBuf, bool) {
    if let (Some(exe_dir), Some(exe_name)) = (
        current_exe.parent(),
        current_exe.file_name().and_then(|s| s.to_str()),
    ) {
        let dir_is_symlink = std::fs::symlink_metadata(exe_dir)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(true);
        if !dir_is_symlink {
            let probe = exe_dir.join(format!(
                ".mse-write-probe-{}.tmp",
                uuid::Uuid::new_v4().simple()
            ));
            if std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&probe)
                .is_ok()
            {
                let _ = std::fs::remove_file(&probe);
                return (exe_dir.join(format!("{exe_name}.{staging_id}.new")), true);
            }
        }
    }

    (
        paths
            .root
            .join("temp")
            .join("updates")
            .join(staging_id)
            .join(asset_name),
        false,
    )
}

#[cfg(windows)]
fn apply_downloaded_update(
    install_mode: InstallMode,
    destination: &Path,
    current_exe: Option<&Path>,
    can_replace_in_place: bool,
) -> Result<(), LauncherErrorPayload> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    match install_mode {
        InstallMode::Installer => {
            std::process::Command::new(destination)
                .spawn()
                .map_err(|e| {
                    LauncherErrorPayload::new(
                        ErrorCode::LaunchFailed,
                        format!("Gagal menjalankan installer pembaruan: {e}"),
                    )
                })?;
        }
        InstallMode::Portable => {
            if can_replace_in_place {
                if let Some(target_exe) = current_exe {
                    let pid = std::process::id().to_string();
                    let script = r#"
$pidVal = [int]$env:MSE_UPDATE_PID
for ($i = 0; $i -lt 80; $i++) {
    if (-not (Get-Process -Id $pidVal -ErrorAction SilentlyContinue)) { break }
    Start-Sleep -Milliseconds 250
}
$moved = $false
for ($i = 0; $i -lt 10; $i++) {
    try {
        Move-Item -LiteralPath $env:MSE_UPDATE_SRC -Destination $env:MSE_UPDATE_DST -Force
        $moved = $true
        break
    } catch {
        Start-Sleep -Milliseconds 250
    }
}
if ($moved -and (Test-Path -LiteralPath $env:MSE_UPDATE_DST)) {
    Start-Process -FilePath $env:MSE_UPDATE_DST
} elseif (Test-Path -LiteralPath $env:MSE_UPDATE_SRC) {
    Start-Process -FilePath $env:MSE_UPDATE_SRC
}
"#;
                    let mut cmd = std::process::Command::new("powershell.exe");
                    cmd.env("MSE_UPDATE_PID", pid)
                        .env("MSE_UPDATE_SRC", destination)
                        .env("MSE_UPDATE_DST", target_exe)
                        .creation_flags(CREATE_NO_WINDOW)
                        .args([
                            "-NoProfile",
                            "-NonInteractive",
                            "-ExecutionPolicy",
                            "Bypass",
                            "-Command",
                            script,
                        ]);
                    cmd.spawn().map_err(|e| {
                        LauncherErrorPayload::new(
                            ErrorCode::LaunchFailed,
                            format!("Gagal mengganti berkas portable pembaruan: {e}"),
                        )
                    })?;
                    return Ok(());
                }
            }
            std::process::Command::new(destination)
                .spawn()
                .map_err(|e| {
                    LauncherErrorPayload::new(
                        ErrorCode::LaunchFailed,
                        format!("Gagal membuka versi portable terbaru: {e}"),
                    )
                })?;
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn apply_downloaded_update(
    _install_mode: InstallMode,
    destination: &Path,
    _current_exe: Option<&Path>,
    _can_replace_in_place: bool,
) -> Result<(), LauncherErrorPayload> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(destination) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(destination, perms);
        }
    }
    std::process::Command::new(destination)
        .spawn()
        .map_err(|e| {
            LauncherErrorPayload::new(
                ErrorCode::LaunchFailed,
                format!("Gagal menjalankan pembaruan: {e}"),
            )
        })?;
    Ok(())
}

#[tauri::command]
pub async fn updater_install(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    download_url: String,
    asset_name: String,
    _sha256: Option<String>,
    _size_bytes: Option<u64>,
) -> Result<serde_json::Value, LauncherErrorPayload> {
    let requested_url = validate_update_url(&download_url)?;
    let requested_asset = validate_asset_filename(&asset_name)?;
    let install_mode = detect_current_install_mode();
    let current_version = env!("CARGO_PKG_VERSION");

    // Require updates to match the cached SHA-256-verified official manifest.
    let cached_verified = state.verified_update.read().await.clone();
    let verified = match cached_verified {
        Some(ref res)
            if res.update_available
                && res.install_mode == install_mode
                && res.download_url.as_deref() == Some(requested_url.as_str())
                && res.asset_name.as_deref() == Some(requested_asset.as_str())
                && is_valid_sha256_hex(res.sha256.as_deref()) =>
        {
            res.clone()
        }
        _ => {
            let http = HttpClient::new();
            let fresh = try_fetch_any_verified_manifest(&http, current_version, install_mode)
                .await
                .ok_or_else(|| {
                    LauncherErrorPayload::network(
                        "Manifes pembaruan terverifikasi (latest.json) tidak tersedia.",
                    )
                })?;
            *state.verified_update.write().await = Some(fresh.clone());
            fresh
        }
    };

    if !verified.update_available
        || verified.install_mode != install_mode
        || verified.download_url.as_deref() != Some(requested_url.as_str())
        || verified.asset_name.as_deref() != Some(requested_asset.as_str())
    {
        return Err(LauncherErrorPayload::invalid_input(
            "Permintaan pembaruan tidak cocok dengan rilis terbaru yang terverifikasi.",
        ));
    }

    let verified_sha256 = verified
        .sha256
        .clone()
        .filter(|s| is_valid_sha256_hex(Some(s.as_str())))
        .ok_or_else(|| {
            LauncherErrorPayload::network(
                "Checksum SHA-256 rilis tidak valid atau tidak tersedia. Pembaruan dibatalkan.",
            )
        })?;
    let verified_url = validate_update_url(
        verified
            .download_url
            .as_deref()
            .unwrap_or(requested_url.as_str()),
    )?;
    let safe_asset_name = validate_asset_filename(
        verified
            .asset_name
            .as_deref()
            .unwrap_or(requested_asset.as_str()),
    )?;
    let verified_size = verified.size_bytes;

    let task_id = uuid::Uuid::new_v4().to_string();
    let staging_id = uuid::Uuid::new_v4().simple().to_string();
    let current_exe = std::env::current_exe().ok();
    let (destination, can_replace_in_place) = match install_mode {
        InstallMode::Installer => (
            state
                .paths
                .root
                .join("temp")
                .join("updates")
                .join(&staging_id)
                .join(&safe_asset_name),
            false,
        ),
        InstallMode::Portable => match current_exe.as_deref() {
            Some(exe) => {
                portable_staging_destination(exe, &state.paths, &safe_asset_name, &staging_id)
            }
            None => (
                state
                    .paths
                    .root
                    .join("temp")
                    .join("updates")
                    .join(&staging_id)
                    .join(&safe_asset_name),
                false,
            ),
        },
    };

    let task_id_clone = task_id.clone();
    let cancelled = crate::commands::launch::register_task(&task_id);
    let downloader = state.downloader.clone();
    let app_clone = app.clone();
    let download_url_str = verified_url.to_string();

    tokio::spawn(async move {
        let _guard = crate::commands::launch::ActiveTaskGuard::new(task_id_clone.clone());

        let initial_total = verified_size.unwrap_or(1);
        let _ = app_clone.emit(
            "task:progress",
            json!({
                "taskId": task_id_clone,
                "progress": {
                    "stage": "Mengunduh pembaruan",
                    "completed": 0,
                    "total": initial_total,
                }
            }),
        );

        let app_prog = app_clone.clone();
        let tid_prog = task_id_clone.clone();
        let progress_cb = move |p: launcher_download::DownloadProgress| {
            let _ = app_prog.emit(
                "task:progress",
                json!({
                    "taskId": tid_prog,
                    "progress": {
                        "stage": "Mengunduh pembaruan",
                        "completed": p.completed_bytes,
                        "total": p.total_bytes,
                    }
                }),
            );
        };

        let tasks = vec![DownloadTask {
            url: download_url_str,
            destination: destination.clone(),
            expected_sha1: None,
            expected_sha256: Some(verified_sha256),
            expected_size: verified_size,
        }];

        if let Err(e) = downloader.download_all(tasks, progress_cb, cancelled).await {
            let _ = app_clone.emit("task:error", super::task_error_payload(&task_id_clone, &e));
            return;
        }

        if let Err(e) = apply_downloaded_update(
            install_mode,
            &destination,
            current_exe.as_deref(),
            can_replace_in_place,
        ) {
            let _ = app_clone.emit("task:error", super::task_error_payload(&task_id_clone, &e));
            return;
        }

        let _ = app_clone.emit(
            "task:done",
            json!({
                "taskId": task_id_clone,
                "result": {
                    "installMode": install_mode,
                    "assetName": safe_asset_name,
                }
            }),
        );

        tokio::time::sleep(Duration::from_millis(250)).await;
        app_clone.exit(0);
    });

    Ok(json!({ "taskId": task_id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_install_mode_portable_filename_and_markers() {
        let root = std::env::temp_dir().join(format!("mcl-upd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let portable_exe = root.join("MSE.Launcher_1.1.0_Portable.exe");
        assert_eq!(
            detect_install_mode_for_path(&portable_exe),
            InstallMode::Portable
        );

        let generic_dir = root.join("custom");
        std::fs::create_dir_all(&generic_dir).unwrap();
        let generic_exe = generic_dir.join("MSE Launcher.exe");
        assert_eq!(
            detect_install_mode_for_path(&generic_exe),
            InstallMode::Portable
        );

        std::fs::write(generic_dir.join("uninstall.exe"), b"stub").unwrap();
        assert_eq!(
            detect_install_mode_for_path(&generic_exe),
            InstallMode::Installer
        );

        // Explicit portable.txt marker takes precedence over uninstall.exe.
        std::fs::write(generic_dir.join("portable.txt"), b"1").unwrap();
        assert_eq!(
            detect_install_mode_for_path(&generic_exe),
            InstallMode::Portable
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_detect_install_mode_program_paths() {
        let p = Path::new(r"C:\Users\Player\AppData\Local\Programs\MSE Launcher\MSE Launcher.exe");
        assert_eq!(detect_install_mode_for_path(p), InstallMode::Installer);

        let pf = Path::new(r"C:\Program Files\MSE Launcher\MSE Launcher.exe");
        assert_eq!(detect_install_mode_for_path(pf), InstallMode::Installer);
    }

    #[test]
    fn test_is_newer_version_semver_and_prerelease() {
        assert!(is_newer_version("1.0.0", "1.1.0"));
        assert!(is_newer_version("1.0.0-beta", "1.0.0"));
        assert!(is_newer_version("1.0.0-beta", "1.1.0"));
        assert!(is_newer_version("1.0.0-beta.2", "1.0.0-beta.10"));
        assert!(is_newer_version("1.0.0+build1", "1.0.1"));
        assert!(!is_newer_version("1.0.0-beta.10", "1.0.0-beta.2"));
        assert!(!is_newer_version("1.1.0", "1.1.0"));
        assert!(!is_newer_version("1.1.0", "1.0.0-beta"));
        assert!(!is_newer_version("1.2.0", "v1.1.9"));
        assert!(is_newer_version("v1.0.0", "v1.1.0"));
    }

    #[test]
    fn test_select_release_and_parse_prerelease_manifest() {
        let prerelease = GitHubRelease {
            tag_name: "v1.1.0-beta".to_string(),
            draft: false,
            prerelease: true,
            body: Some("Beta notes".to_string()),
            assets: vec![GitHubAsset {
                name: "latest.json".to_string(),
                browser_download_url:
                    "https://github.com/IrfanSyahid118/MSE-Launcher/releases/download/v1.1.0-beta/latest.json"
                        .to_string(),
                size: Some(400),
            }],
        };
        let draft = GitHubRelease {
            tag_name: "v1.2.0".to_string(),
            draft: true,
            prerelease: false,
            body: None,
            assets: vec![],
        };
        let list = [draft, prerelease];
        let picked = select_release(&list).expect("should fall back to prerelease when no stable");
        assert_eq!(picked.tag_name, "v1.1.0-beta");

        let manifest = LatestManifest {
            version: "1.1.0-beta".to_string(),
            notes: None,
            platforms: Some(std::collections::HashMap::from([(
                current_platform_key().to_string(),
                PlatformManifest {
                    installer: Some(ManifestAsset {
                        name: "MSE.Launcher_1.1.0-beta_x64-setup.exe".to_string(),
                        url: "https://github.com/IrfanSyahid118/MSE-Launcher/releases/download/v1.1.0-beta/MSE.Launcher_1.1.0-beta_x64-setup.exe".to_string(),
                        sha256: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
                        size: Some(12345),
                    }),
                    portable: None,
                },
            )])),
        };
        let parsed = parse_manifest_result(manifest, "1.0.0-beta", InstallMode::Installer)
            .expect("manifest should parse");
        assert!(parsed.update_available);
        assert!(parsed.sha256.is_some());
    }

    #[test]
    fn test_select_release_prefers_higher_semver_even_when_prerelease() {
        let older_stable = GitHubRelease {
            tag_name: "v1.0.0".to_string(),
            draft: false,
            prerelease: false,
            body: None,
            assets: vec![],
        };
        let newer_beta = GitHubRelease {
            tag_name: "v1.2.2-beta".to_string(),
            draft: false,
            prerelease: true,
            body: None,
            assets: vec![],
        };
        let list = [older_stable, newer_beta];
        let picked = select_release(&list).expect("should select highest semver release");
        assert_eq!(picked.tag_name, "v1.2.2-beta");
    }

    #[test]
    fn test_validate_update_url_allows_only_official_github_releases() {
        assert!(validate_update_url(
            "https://github.com/IrfanSyahid118/MSE-Launcher/releases/download/v1.1.0/MSE.Launcher_1.1.0_Portable.exe"
        )
        .is_ok());
        assert!(validate_update_url(
            "https://objects.githubusercontent.com/github-production-release-asset-123/file.exe"
        )
        .is_err());
        assert!(validate_update_url(
            "http://github.com/IrfanSyahid118/MSE-Launcher/releases/download/v1.1.0/a.exe"
        )
        .is_err());
        assert!(validate_update_url(
            "https://github.com/otheruser/otherrepo/releases/download/v1.1.0/evil.exe"
        )
        .is_err());
        assert!(validate_update_url("https://evil.example.com/payload.exe").is_err());
    }

    #[test]
    fn test_valid_sha256_hex_requirement() {
        assert!(is_valid_sha256_hex(Some(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        )));
        assert!(!is_valid_sha256_hex(None));
        assert!(!is_valid_sha256_hex(Some("")));
        assert!(!is_valid_sha256_hex(Some("abcd")));
        assert!(!is_valid_sha256_hex(Some(
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
        )));
    }

    #[test]
    fn test_h6_parse_companion_checksum_formats() {
        let hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let asset = "MSE.Launcher_1.1.0_Portable.exe";
        assert_eq!(
            parse_sha256_from_checksum_text(hash, asset).as_deref(),
            Some(hash)
        );
        let sums = format!(
            "0000000000000000000000000000000000000000000000000000000000000000  other.exe\n{hash} *{asset}\n"
        );
        assert_eq!(
            parse_sha256_from_checksum_text(&sums, asset).as_deref(),
            Some(hash)
        );
        let bsd = format!("SHA256 ({asset}) = {hash}\n");
        assert_eq!(
            parse_sha256_from_checksum_text(&bsd, asset).as_deref(),
            Some(hash)
        );
        assert!(parse_sha256_from_checksum_text("invalid-checksum", asset).is_none());
    }
}
