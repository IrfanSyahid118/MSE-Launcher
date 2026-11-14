use crate::models::{LoaderProfile, LoaderVersionInfo};
use launcher_net::HttpClient;
use launcher_types::LauncherErrorPayload;
use serde::Deserialize;

pub const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2/versions/loader";
pub const FABRIC_GAME_URL: &str = "https://meta.fabricmc.net/v2/versions/game";

#[derive(Debug, Deserialize)]
struct FabricGameEntry {
    version: String,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderObject {
    version: String,
    stable: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoaderObject,
}

pub async fn fetch_fabric_game_versions(
    http: &HttpClient,
) -> Result<Vec<String>, LauncherErrorPayload> {
    let entries: Vec<FabricGameEntry> = http.get_json(FABRIC_GAME_URL).await?;
    Ok(entries.into_iter().map(|e| e.version).collect())
}

pub async fn fetch_fabric_loader_versions(
    http: &HttpClient,
    minecraft_version: &str,
) -> Result<Vec<LoaderVersionInfo>, LauncherErrorPayload> {
    let url = format!("{}/{}", FABRIC_META_URL, minecraft_version);
    let entries: Vec<FabricLoaderEntry> = http.get_json(&url).await?;

    Ok(entries
        .into_iter()
        .map(|e| LoaderVersionInfo {
            version: e.loader.version,
            stable: e.loader.stable.unwrap_or(true),
        })
        .collect())
}

pub async fn fetch_fabric_profile(
    http: &HttpClient,
    minecraft_version: &str,
    loader_version: &str,
) -> Result<LoaderProfile, LauncherErrorPayload> {
    let url = format!(
        "{}/{}/{}/profile/json",
        FABRIC_META_URL, minecraft_version, loader_version
    );
    http.get_json(&url).await
}
