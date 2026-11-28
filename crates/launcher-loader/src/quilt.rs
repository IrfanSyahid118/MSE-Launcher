use crate::models::{LoaderProfile, LoaderVersionInfo};
use launcher_net::HttpClient;
use launcher_types::LauncherErrorPayload;
use serde::Deserialize;

pub const QUILT_META_URL: &str = "https://meta.quiltmc.org/v3/versions/loader";
pub const QUILT_GAME_URL: &str = "https://meta.quiltmc.org/v3/versions/game";

#[derive(Debug, Deserialize)]
struct QuiltGameEntry {
    version: String,
}

#[derive(Debug, Deserialize)]
struct QuiltLoaderObject {
    version: String,
}

#[derive(Debug, Deserialize)]
struct QuiltLoaderEntry {
    loader: QuiltLoaderObject,
}

pub async fn fetch_quilt_game_versions(
    http: &HttpClient,
) -> Result<Vec<String>, LauncherErrorPayload> {
    let entries: Vec<QuiltGameEntry> = http.get_json(QUILT_GAME_URL).await?;
    Ok(entries.into_iter().map(|e| e.version).collect())
}

pub async fn fetch_quilt_loader_versions(
    http: &HttpClient,
    minecraft_version: &str,
) -> Result<Vec<LoaderVersionInfo>, LauncherErrorPayload> {
    launcher_security::validate_safe_single_component(minecraft_version)?;
    let url = format!("{}/{}", QUILT_META_URL, minecraft_version);
    let entries: Vec<QuiltLoaderEntry> = http.get_json(&url).await?;

    Ok(entries
        .into_iter()
        .map(|e| LoaderVersionInfo {
            stable: !e.loader.version.contains('-'),
            version: e.loader.version,
        })
        .collect())
}

pub async fn fetch_quilt_profile(
    http: &HttpClient,
    minecraft_version: &str,
    loader_version: &str,
) -> Result<LoaderProfile, LauncherErrorPayload> {
    launcher_security::validate_safe_single_component(minecraft_version)?;
    launcher_security::validate_safe_single_component(loader_version)?;
    let url = format!(
        "{}/{}/{}/profile/json",
        QUILT_META_URL, minecraft_version, loader_version
    );
    http.get_json(&url).await
}
