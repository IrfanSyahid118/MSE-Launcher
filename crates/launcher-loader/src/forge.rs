use crate::models::{compare_loader_versions, LoaderVersionInfo};
use launcher_net::HttpClient;
use launcher_types::LauncherErrorPayload;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub const FORGE_METADATA_URL: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";

static FORGE_VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<version>\s*([^<\s]+)\s*</version>").expect("valid regex"));

pub struct ForgeIndex {
    pub game_versions: Vec<String>,
    pub versions_by_game: HashMap<String, Vec<String>>,
}

pub fn parse_forge_metadata(xml: &str) -> ForgeIndex {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    for cap in FORGE_VERSION_REGEX.captures_iter(xml) {
        if let Some(v_str) = cap.get(1).map(|m| m.as_str()) {
            if let Some((game, forge)) = v_str.split_once('-') {
                if !game.is_empty() && !forge.is_empty() {
                    map.entry(game.to_string())
                        .or_default()
                        .push(forge.to_string());
                }
            }
        }
    }

    for list in map.values_mut() {
        list.sort_by(|a, b| compare_loader_versions(a, b));
    }

    let mut game_versions: Vec<String> = map.keys().cloned().collect();
    game_versions.sort_by(|a, b| compare_loader_versions(a, b));

    ForgeIndex {
        game_versions,
        versions_by_game: map,
    }
}

pub async fn fetch_forge_index(http: &HttpClient) -> Result<ForgeIndex, LauncherErrorPayload> {
    let response = http.get(FORGE_METADATA_URL).await?;
    let xml = response.text().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal membaca metadata Forge XML: {}", e))
    })?;

    Ok(parse_forge_metadata(&xml))
}

pub fn forge_versions_for_game(
    index: &ForgeIndex,
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
