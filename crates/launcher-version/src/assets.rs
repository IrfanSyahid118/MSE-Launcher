use launcher_download::DownloadTask;
use launcher_types::LauncherErrorPayload;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangAssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MojangAssetIndex {
    #[serde(rename = "virtual", default)]
    pub is_virtual: Option<bool>,
    #[serde(default)]
    pub map_to_resources: Option<bool>,
    pub objects: HashMap<String, MojangAssetObject>,
}

fn is_valid_asset_hash(hash: &str) -> bool {
    hash.len() >= 2 && hash.len() <= 128 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn build_asset_download_tasks(
    index: &MojangAssetIndex,
    assets_dir: &Path,
) -> Vec<DownloadTask> {
    let mut tasks = Vec::with_capacity(index.objects.len());
    let objects_dir = assets_dir.join("objects");

    for obj in index.objects.values() {
        if !is_valid_asset_hash(&obj.hash) {
            continue;
        }
        let prefix = &obj.hash[0..2];
        let Ok(prefix_dir) = launcher_security::safe_subpath(&objects_dir, prefix) else {
            continue;
        };
        let Ok(destination) = launcher_security::safe_subpath(&prefix_dir, &obj.hash) else {
            continue;
        };
        let url = format!(
            "https://resources.download.minecraft.net/{}/{}",
            prefix, obj.hash
        );

        tasks.push(DownloadTask {
            url,
            destination,
            expected_sha1: Some(obj.hash.clone()),
            expected_sha256: None,
            expected_size: Some(obj.size),
        });
    }

    tasks
}

fn safe_asset_target(root: &Path, logical_path: &str) -> Result<PathBuf, LauncherErrorPayload> {
    let normalized = logical_path.trim_start_matches("./");
    launcher_security::safe_subpath(root, normalized)
}

pub fn resolve_game_assets_dir(
    index: &MojangAssetIndex,
    index_id: &str,
    assets_dir: &Path,
    game_dir: &Path,
) -> Result<PathBuf, LauncherErrorPayload> {
    if index.map_to_resources.unwrap_or(false) {
        Ok(game_dir.join("resources"))
    } else if index.is_virtual.unwrap_or(false) {
        launcher_security::validate_safe_single_component(index_id)?;
        launcher_security::safe_subpath(&assets_dir.join("virtual"), index_id)
    } else {
        Ok(assets_dir.to_path_buf())
    }
}

pub fn prepare_legacy_assets_from_index(
    index: &MojangAssetIndex,
    index_id: &str,
    assets_dir: &Path,
    game_dir: &Path,
) -> Result<PathBuf, LauncherErrorPayload> {
    let is_legacy = index.map_to_resources.unwrap_or(false) || index.is_virtual.unwrap_or(false);
    if !is_legacy {
        return Ok(assets_dir.to_path_buf());
    }

    let target_root = resolve_game_assets_dir(index, index_id, assets_dir, game_dir)?;
    let objects_dir = assets_dir.join("objects");

    for (logical_path, obj) in &index.objects {
        if !is_valid_asset_hash(&obj.hash) {
            continue;
        }
        let dest = safe_asset_target(&target_root, logical_path)?;
        let prefix = &obj.hash[0..2];
        let src = objects_dir.join(prefix).join(&obj.hash);
        if !src.exists() {
            continue;
        }

        let up_to_date = std::fs::symlink_metadata(&dest)
            .map(|m| m.is_file() && !m.file_type().is_symlink() && m.len() == obj.size)
            .unwrap_or(false);
        if up_to_date {
            continue;
        }

        if let Some(parent) = dest.parent() {
            launcher_security::ensure_descendants_no_symlink(&target_root, parent)?;
            std::fs::create_dir_all(parent).map_err(|e| {
                LauncherErrorPayload::file_system(format!(
                    "Gagal membuat folder asset legacy: {}",
                    e
                ))
            })?;
        }
        launcher_security::ensure_descendants_no_symlink(&target_root, &dest)?;

        std::fs::copy(&src, &dest).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal menyalin asset legacy: {}", e))
        })?;
    }

    Ok(target_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_n27_maps_virtual_and_map_to_resources_assets() {
        let tmp = tempdir().unwrap();
        let assets_dir = tmp.path().join("assets");
        let game_dir = tmp.path().join("instances").join("inst1").join("minecraft");
        let hash = "aa11bb22cc33dd44ee55ff6677889900aabbccdd";
        let obj_path = assets_dir.join("objects").join("aa").join(hash);
        std::fs::create_dir_all(obj_path.parent().unwrap()).unwrap();
        std::fs::write(&obj_path, b"sound-bytes").unwrap();

        let virtual_json = format!(
            r#"{{"virtual":true,"objects":{{"sound/step/grass1.ogg":{{"hash":"{}","size":11}}}}}}"#,
            hash
        );
        let virtual_idx: MojangAssetIndex = serde_json::from_str(&virtual_json).unwrap();
        let resolved_virtual =
            prepare_legacy_assets_from_index(&virtual_idx, "legacy", &assets_dir, &game_dir)
                .unwrap();
        assert_eq!(resolved_virtual, assets_dir.join("virtual").join("legacy"));
        assert_eq!(
            std::fs::read(resolved_virtual.join("sound/step/grass1.ogg")).unwrap(),
            b"sound-bytes"
        );

        let pre16_json = format!(
            r#"{{"map_to_resources":true,"objects":{{"music/calm1.ogg":{{"hash":"{}","size":11}}}}}}"#,
            hash
        );
        let pre16_idx: MojangAssetIndex = serde_json::from_str(&pre16_json).unwrap();
        let resolved_pre16 =
            prepare_legacy_assets_from_index(&pre16_idx, "pre-1.6", &assets_dir, &game_dir)
                .unwrap();
        assert_eq!(resolved_pre16, game_dir.join("resources"));
        assert_eq!(
            std::fs::read(resolved_pre16.join("music/calm1.ogg")).unwrap(),
            b"sound-bytes"
        );

        let evil_json = format!(
            r#"{{"virtual":true,"objects":{{"../../evil.ogg":{{"hash":"{}","size":11}}}}}}"#,
            hash
        );
        let evil_idx: MojangAssetIndex = serde_json::from_str(&evil_json).unwrap();
        assert!(
            prepare_legacy_assets_from_index(&evil_idx, "evil", &assets_dir, &game_dir).is_err()
        );
        assert!(!tmp.path().join("evil.ogg").exists());
    }
}
