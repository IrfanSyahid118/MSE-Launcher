use crate::models::{compare_loader_versions, LoaderVersionInfo};
use launcher_net::HttpClient;
use launcher_types::LauncherErrorPayload;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub const NEOFORGE_METADATA_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

static NEOFORGE_GAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)\.(\d+)(?:\.|$)").expect("valid regex"));
static NEOFORGE_VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<version>\s*([^<\s]+)\s*</version>").expect("valid regex"));

pub struct NeoForgeIndex {
    pub game_versions: Vec<String>,
    pub versions_by_game: HashMap<String, Vec<String>>,
}

pub fn neoforge_to_game_version(version: &str) -> Option<String> {
    let caps = NEOFORGE_GAME_REGEX.captures(version)?;
    let major = caps.get(1)?.as_str();
    let minor = caps.get(2)?.as_str();

    if minor == "0" {
        Some(format!("1.{}", major))
    } else {
        Some(format!("1.{}.{}", major, minor))
    }
}

pub fn parse_neoforge_metadata(xml: &str) -> NeoForgeIndex {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    for cap in NEOFORGE_VERSION_REGEX.captures_iter(xml) {
        if let Some(v_str) = cap.get(1).map(|m| m.as_str()) {
            if let Some(game) = neoforge_to_game_version(v_str) {
                map.entry(game).or_default().push(v_str.to_string());
            }
        }
    }

    for list in map.values_mut() {
        list.sort_by(|a, b| compare_loader_versions(a, b));
    }

    let mut game_versions: Vec<String> = map.keys().cloned().collect();
    game_versions.sort_by(|a, b| compare_loader_versions(a, b));

    NeoForgeIndex {
        game_versions,
        versions_by_game: map,
    }
}

pub async fn fetch_neoforge_index(
    http: &HttpClient,
) -> Result<NeoForgeIndex, LauncherErrorPayload> {
    let response = http.get(NEOFORGE_METADATA_URL).await?;
    let xml = response.text().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal membaca metadata NeoForge XML: {}", e))
    })?;

    Ok(parse_neoforge_metadata(&xml))
}

pub fn neoforge_versions_for_game(
    index: &NeoForgeIndex,
    minecraft_version: &str,
) -> Vec<LoaderVersionInfo> {
    if let Some(list) = index.versions_by_game.get(minecraft_version) {
        list.iter()
            .map(|v| LoaderVersionInfo {
                version: v.clone(),
                stable: !v.contains("beta") && !v.contains("alpha"),
            })
            .collect()
    } else {
        Vec::new()
    }
}
