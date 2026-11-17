use crate::state::AppState;
use launcher_types::{
    Account, AccountType, DeviceCodeInfo, InstallSkinResourcePackRequest,
    InstallSkinResourcePackResult, LauncherErrorPayload, SetActiveCapeRequest, SettingsPatch,
    SyncMinecraftProfileRequest, UpdateAccountSkinRequest,
};

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let clean = input.trim_matches(|c: char| c.is_whitespace() || c == '=');
    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in clean.as_bytes() {
        let val = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk.first().copied().unwrap_or(0) as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        let c0 = TABLE.get((triple >> 18) & 0x3F).copied().unwrap_or(b'A') as char;
        let c1 = TABLE.get((triple >> 12) & 0x3F).copied().unwrap_or(b'A') as char;
        out.push(c0);
        out.push(c1);
        if chunk.len() > 1 {
            let c2 = TABLE.get((triple >> 6) & 0x3F).copied().unwrap_or(b'A') as char;
            out.push(c2);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            let c3 = TABLE.get(triple & 0x3F).copied().unwrap_or(b'A') as char;
            out.push(c3);
        } else {
            out.push('=');
        }
    }
    out
}

const MAX_SKIN_PNG_BYTES: usize = 2 * 1024 * 1024;
const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

fn validate_png_bytes(bytes: &[u8]) -> Result<(), LauncherErrorPayload> {
    if bytes.is_empty() || bytes.len() > MAX_SKIN_PNG_BYTES {
        return Err(LauncherErrorPayload::invalid_input(
            "Ukuran berkas PNG skin/jubah tidak valid (maksimum 2 MB).",
        ));
    }
    if !bytes.starts_with(PNG_MAGIC) {
        return Err(LauncherErrorPayload::invalid_input(
            "Berkas skin/jubah harus berformat PNG yang valid.",
        ));
    }
    Ok(())
}

fn read_cached_skin_file(
    skins_dir: &std::path::Path,
    account_id: &str,
    suffix: &str,
) -> Option<Vec<u8>> {
    launcher_security::validate_instance_id(account_id).ok()?;
    let target =
        launcher_security::safe_subpath(skins_dir, &format!("{}{}", account_id, suffix)).ok()?;
    launcher_security::ensure_descendants_no_symlink(skins_dir, &target).ok()?;
    let meta = std::fs::symlink_metadata(&target).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    std::fs::read(&target).ok()
}

fn write_skin_file_safe(
    skins_dir: &std::path::Path,
    target: &std::path::Path,
    bytes: &[u8],
) -> Result<(), LauncherErrorPayload> {
    launcher_security::ensure_secure_dir(skins_dir, true)?;
    launcher_security::ensure_descendants_no_symlink(skins_dir, target)?;
    let tmp = skins_dir.join(format!(".skin.tmp-{}", uuid::Uuid::new_v4().simple()));
    {
        use std::io::Write;
        let mut f = launcher_security::safe_create_new_file(&tmp, true)?;
        if let Err(e) = f.write_all(bytes) {
            let _ = std::fs::remove_file(&tmp);
            return Err(LauncherErrorPayload::file_system(format!(
                "Gagal menulis berkas skin sementara: {}",
                e
            )));
        }
    }
    std::fs::rename(&tmp, target).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        LauncherErrorPayload::file_system(format!("Gagal menyimpan berkas skin: {}", e))
    })
}

pub fn enrich_account(acc: &mut Account, skins_dir: &std::path::Path) {
    if let Some(bytes) = read_cached_skin_file(skins_dir, &acc.id, ".png") {
        acc.skin_url = Some(format!("data:image/png;base64,{}", encode_base64(&bytes)));
    } else if let Some(ref mut url) = acc.skin_url {
        if let Some(rest) = url.strip_prefix("http://") {
            *url = format!("https://{}", rest);
        } else if !url.starts_with("data:") && !url.starts_with("https://") {
            acc.skin_url = None;
        }
    }

    if let Some(bytes) = read_cached_skin_file(skins_dir, &acc.id, "_cape.png") {
        acc.cape_url = Some(format!("data:image/png;base64,{}", encode_base64(&bytes)));
    } else if let Some(ref mut url) = acc.cape_url {
        if let Some(rest) = url.strip_prefix("http://") {
            *url = format!("https://{}", rest);
        } else if !url.starts_with("data:") && !url.starts_with("https://") {
            acc.cape_url = None;
        }
    }

    for c in &mut acc.capes {
        if let Some(rest) = c.url.strip_prefix("http://") {
            c.url = format!("https://{}", rest);
        }
    }
}

#[tauri::command]
pub fn accounts_list(state: tauri::State<AppState>) -> Result<Vec<Account>, LauncherErrorPayload> {
    let mut list = state.accounts.list();
    let skins_dir = state.paths.root.join("skins");

    for acc in &mut list {
        enrich_account(acc, &skins_dir);
    }
    Ok(list)
}

#[tauri::command]
pub fn accounts_set_active(
    state: tauri::State<AppState>,
    id: String,
) -> Result<(), LauncherErrorPayload> {
    launcher_security::validate_instance_id(&id)?;
    if state.accounts.get(&id).is_none() {
        return Err(LauncherErrorPayload::not_found("Akun tidak ditemukan."));
    }
    state.settings.set(SettingsPatch {
        active_account_id: Some(Some(id)),
        ..Default::default()
    })?;
    Ok(())
}

pub(crate) fn update_skin_with_store(
    accounts: &launcher_storage::AccountsStore,
    skins_dir: &std::path::Path,
    req: UpdateAccountSkinRequest,
) -> Result<Account, LauncherErrorPayload> {
    launcher_security::validate_instance_id(&req.account_id)?;
    let existing = accounts
        .get(&req.account_id)
        .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;
    launcher_security::validate_instance_id(&existing.id)?;

    let skin_path = launcher_security::safe_subpath(skins_dir, &format!("{}.png", existing.id))?;
    let cape_path =
        launcher_security::safe_subpath(skins_dir, &format!("{}_cape.png", existing.id))?;

    let (skin_bytes, saved_url) = if req.skin_data.starts_with("data:image/") {
        let (_, b64) = req.skin_data.split_once(',').ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Format data URL skin tidak valid.")
        })?;
        let bytes = decode_base64(b64).ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Payload base64 skin tidak valid.")
        })?;
        validate_png_bytes(&bytes)?;
        let data_url = format!("data:image/png;base64,{}", encode_base64(&bytes));
        (Some(bytes), data_url)
    } else if req.skin_data.starts_with("https://") || req.skin_data.starts_with("http://") {
        (None, req.skin_data)
    } else {
        return Err(LauncherErrorPayload::invalid_input(
            "Sumber skin harus berupa data URL PNG atau URL HTTP(S), bukan path file lokal.",
        ));
    };

    let (cape_bytes, saved_cape) = if let Some(ref cape_data) = req.cape_data {
        if cape_data.starts_with("data:image/") {
            let (_, b64) = cape_data.split_once(',').ok_or_else(|| {
                LauncherErrorPayload::invalid_input("Format data URL cape tidak valid.")
            })?;
            let bytes = decode_base64(b64).ok_or_else(|| {
                LauncherErrorPayload::invalid_input("Payload base64 cape tidak valid.")
            })?;
            validate_png_bytes(&bytes)?;
            let data_url = format!("data:image/png;base64,{}", encode_base64(&bytes));
            (Some(bytes), Some(data_url))
        } else if cape_data.starts_with("https://") || cape_data.starts_with("http://") {
            (None, Some(cape_data.clone()))
        } else {
            return Err(LauncherErrorPayload::invalid_input(
                "Sumber jubah harus berupa data URL PNG atau URL HTTP(S), bukan path file lokal.",
            ));
        }
    } else {
        (None, None)
    };

    if let Some(ref bytes) = skin_bytes {
        write_skin_file_safe(skins_dir, &skin_path, bytes)?;
    } else {
        let _ = launcher_security::safe_remove_file_within(skins_dir, &skin_path);
    }

    if let Some(ref bytes) = cape_bytes {
        write_skin_file_safe(skins_dir, &cape_path, bytes)?;
    } else {
        let _ = launcher_security::safe_remove_file_within(skins_dir, &cape_path);
    }

    let mut updated =
        accounts.update_skin(&existing.id, Some(saved_url), req.skin_variant, saved_cape)?;
    enrich_account(&mut updated, skins_dir);

    Ok(updated)
}

#[tauri::command]
pub fn accounts_update_skin(
    state: tauri::State<AppState>,
    req: UpdateAccountSkinRequest,
) -> Result<Account, LauncherErrorPayload> {
    let skins_dir = state.paths.root.join("skins");
    update_skin_with_store(&state.accounts, &skins_dir, req)
}

#[tauri::command]
pub fn auth_create_offline(
    state: tauri::State<AppState>,
    username: String,
) -> Result<Account, LauncherErrorPayload> {
    let account = state.accounts.create_offline(&username)?;

    let current_settings = state.settings.get();
    if current_settings.active_account_id.is_none() {
        let _ = state.settings.set(SettingsPatch {
            active_account_id: Some(Some(account.id.clone())),
            ..Default::default()
        });
    }

    Ok(account)
}

#[tauri::command]
pub async fn auth_begin_microsoft(
    state: tauri::State<'_, AppState>,
) -> Result<DeviceCodeInfo, LauncherErrorPayload> {
    state.auth.begin_login().await
}

#[tauri::command]
pub async fn auth_complete_microsoft(
    state: tauri::State<'_, AppState>,
    req: DeviceCodeInfo,
    task_id: Option<String>,
) -> Result<Account, LauncherErrorPayload> {
    let (_guard, cancel_flag) = if let Some(tid) = task_id.filter(|s| !s.trim().is_empty()) {
        let flag = crate::commands::launch::register_task(&tid);
        (
            Some(crate::commands::launch::ActiveTaskGuard::new(tid)),
            Some(flag),
        )
    } else {
        (None, None)
    };

    let account = state
        .auth
        .complete_login_with_cancel(req, cancel_flag)
        .await?;

    let current_settings = state.settings.get();
    if current_settings.active_account_id.is_none() {
        let _ = state.settings.set(SettingsPatch {
            active_account_id: Some(Some(account.id.clone())),
            ..Default::default()
        });
    }

    Ok(account)
}

#[tauri::command]
pub async fn auth_sign_out(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> Result<(), LauncherErrorPayload> {
    let _ = state.auth.sign_out(&account_id).await;
    state.accounts.remove(&account_id)?;

    if launcher_security::validate_instance_id(&account_id).is_ok() {
        let skins_dir = state.paths.root.join("skins");
        for file in [
            format!("{account_id}.png"),
            format!("{account_id}_cape.png"),
        ] {
            if let Ok(path) = launcher_security::safe_subpath(&skins_dir, &file) {
                let _ = launcher_security::safe_remove_file_within(&skins_dir, &path);
            }
        }
    }

    let current_settings = state.settings.get();
    if current_settings.active_account_id.as_deref() == Some(&account_id) {
        let _ = state.settings.set(SettingsPatch {
            active_account_id: Some(None),
            ..Default::default()
        });
    }

    Ok(())
}

fn resolve_skin_bytes_for_account(
    skins_dir: &std::path::Path,
    account: &Account,
    is_slim: bool,
) -> Result<Vec<u8>, LauncherErrorPayload> {
    let cached = read_cached_skin_file(skins_dir, &account.id, ".png");
    let from_url = cached.or_else(|| {
        account.skin_url.as_ref().and_then(|skin_source| {
            if skin_source.starts_with("data:image/") {
                skin_source
                    .split_once(',')
                    .and_then(|(_, b64)| decode_base64(b64))
                    .filter(|b| !b.is_empty() && b.len() <= MAX_SKIN_PNG_BYTES)
            } else {
                None
            }
        })
    });

    let final_skin_bytes =
        from_url.unwrap_or_else(|| launcher_archive::get_default_skin_bytes(is_slim));
    if final_skin_bytes.is_empty() {
        return Err(LauncherErrorPayload::invalid_input(
            "Data skin tidak ditemukan.",
        ));
    }
    Ok(final_skin_bytes)
}

#[tauri::command]
pub fn accounts_install_skin_resource_pack(
    state: tauri::State<AppState>,
    req: InstallSkinResourcePackRequest,
) -> Result<InstallSkinResourcePackResult, LauncherErrorPayload> {
    launcher_security::validate_instance_id(&req.account_id)?;
    let account = state
        .accounts
        .get(&req.account_id)
        .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;

    let is_slim = account
        .skin_variant
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("slim"))
        .unwrap_or(false);

    let skins_dir = state.paths.root.join("skins");
    let final_skin_bytes = resolve_skin_bytes_for_account(&skins_dir, &account, is_slim)?;

    let all_instances = state.instances.list()?;
    let apply_all =
        req.instance_ids.is_empty() || req.instance_ids.iter().any(|id| id == "all" || id == "*");

    let target_instances: Vec<_> = if apply_all {
        all_instances
    } else {
        all_instances
            .into_iter()
            .filter(|inst| req.instance_ids.contains(&inst.id))
            .collect()
    };

    let mut updated_count = 0;
    let pack_name = "SkinLoader-ResourcePack.zip";

    for inst in target_instances {
        if launcher_security::validate_instance_id(&inst.id).is_err() {
            continue;
        }
        let Ok(inst_dir) = launcher_security::safe_subpath(&state.paths.instances_dir, &inst.id)
        else {
            continue;
        };
        let mc_dir = inst_dir.join("minecraft");
        let rp_dir = mc_dir.join("resourcepacks");
        let rp_file = rp_dir.join(pack_name);
        if launcher_security::ensure_descendants_no_symlink(&state.paths.instances_dir, &rp_file)
            .is_err()
        {
            continue;
        }

        if launcher_archive::build_skin_resourcepack(&final_skin_bytes, &rp_file).is_ok() {
            let options_file = mc_dir.join("options.txt");
            if launcher_security::ensure_descendants_no_symlink(
                &state.paths.instances_dir,
                &options_file,
            )
            .is_ok()
            {
                let _ =
                    launcher_archive::activate_resourcepack_in_options(&options_file, pack_name);
            }
            updated_count += 1;
        }
    }

    Ok(InstallSkinResourcePackResult {
        updated_instances: updated_count,
        pack_file_name: pack_name.to_string(),
    })
}

pub(crate) fn export_skin_resourcepack_to_dir(
    paths: &launcher_platform::DataPaths,
    accounts: &launcher_storage::AccountsStore,
    account_id: &str,
    destination_file_name: &str,
) -> Result<String, LauncherErrorPayload> {
    launcher_security::validate_instance_id(account_id)?;
    let clean_name = launcher_security::validate_single_filename(destination_file_name)?;
    if !clean_name.to_ascii_lowercase().ends_with(".zip") {
        return Err(LauncherErrorPayload::invalid_input(
            "Nama berkas ekspor harus berakhiran .zip tanpa separator folder.",
        ));
    }

    let account = accounts
        .get(account_id)
        .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;

    let is_slim = account
        .skin_variant
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("slim"))
        .unwrap_or(false);

    let skins_dir = paths.root.join("skins");
    let final_skin_bytes = resolve_skin_bytes_for_account(&skins_dir, &account, is_slim)?;

    let exports_dir = paths.root.join("exports");
    launcher_security::ensure_dir_within(&paths.root, &exports_dir, true)?;
    let dest = launcher_security::safe_subpath(&exports_dir, clean_name)?;
    launcher_security::ensure_descendants_no_symlink(&exports_dir, &dest)?;

    launcher_archive::build_skin_resourcepack(&final_skin_bytes, &dest)?;

    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn accounts_export_skin_resource_pack(
    state: tauri::State<AppState>,
    account_id: String,
    destination_path: String,
) -> Result<String, LauncherErrorPayload> {
    export_skin_resourcepack_to_dir(
        &state.paths,
        &state.accounts,
        &account_id,
        &destination_path,
    )
}

#[tauri::command]
pub async fn accounts_sync_minecraft(
    state: tauri::State<'_, AppState>,
    req: SyncMinecraftProfileRequest,
) -> Result<Account, LauncherErrorPayload> {
    let mut updated = state.auth.sync_profile(&req.account_id).await?;
    let skins_dir = state.paths.root.join("skins");
    tokio::task::spawn_blocking(move || {
        enrich_account(&mut updated, &skins_dir);
        updated
    })
    .await
    .map_err(|e| LauncherErrorPayload::internal(format!("Gagal memproses data skin akun: {}", e)))
}

pub(crate) fn set_custom_cape_with_store(
    accounts: &launcher_storage::AccountsStore,
    skins_dir: &std::path::Path,
    account_id: &str,
    custom_data: &str,
) -> Result<Account, LauncherErrorPayload> {
    launcher_security::validate_instance_id(account_id)?;
    let mut acc = accounts
        .get(account_id)
        .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;
    launcher_security::validate_instance_id(&acc.id)?;
    let cape_path = launcher_security::safe_subpath(skins_dir, &format!("{}_cape.png", acc.id))?;

    let bytes = if custom_data.starts_with("data:image/") {
        custom_data
            .split_once(',')
            .and_then(|(_, b64)| decode_base64(b64))
    } else {
        decode_base64(custom_data)
    }
    .ok_or_else(|| LauncherErrorPayload::invalid_input("Format data jubah kustom tidak valid."))?;

    validate_png_bytes(&bytes)?;
    write_skin_file_safe(skins_dir, &cape_path, &bytes)?;

    acc.cape_url = Some(format!("data:image/png;base64,{}", encode_base64(&bytes)));
    acc.active_cape_id = Some("custom".to_string());
    for c in &mut acc.capes {
        c.state = "INACTIVE".to_string();
    }
    accounts.save_account(acc.clone())?;
    enrich_account(&mut acc, skins_dir);
    Ok(acc)
}

#[tauri::command]
pub async fn accounts_set_active_cape(
    state: tauri::State<'_, AppState>,
    req: SetActiveCapeRequest,
) -> Result<Account, LauncherErrorPayload> {
    let skins_dir = state.paths.root.join("skins");
    if let Some(custom_data) = req.custom_cape_data {
        let accounts = state.accounts.clone();
        let account_id = req.account_id;
        return tokio::task::spawn_blocking(move || {
            set_custom_cape_with_store(&accounts, &skins_dir, &account_id, &custom_data)
        })
        .await
        .map_err(|e| {
            LauncherErrorPayload::internal(format!("Gagal memproses jubah kustom: {}", e))
        })?;
    }

    let requested_cape = req
        .cape_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if requested_cape.is_none() {
        if let Some(mut acc) = state.accounts.get(&req.account_id) {
            if acc.account_type == AccountType::Offline {
                acc.cape_url = None;
                acc.active_cape_id = None;
                state.accounts.save_account(acc.clone())?;
                if launcher_security::validate_instance_id(&acc.id).is_ok() {
                    if let Ok(cape_path) =
                        launcher_security::safe_subpath(&skins_dir, &format!("{}_cape.png", acc.id))
                    {
                        let _ = launcher_security::safe_remove_file_within(&skins_dir, &cape_path);
                    }
                }
                enrich_account(&mut acc, &skins_dir);
                return Ok(acc);
            }
        }
    }

    let mut updated = state
        .auth
        .set_active_cape(&req.account_id, requested_cape)
        .await?;
    if updated.active_cape_id.is_none()
        && launcher_security::validate_instance_id(&updated.id).is_ok()
    {
        if let Ok(cape_path) =
            launcher_security::safe_subpath(&skins_dir, &format!("{}_cape.png", updated.id))
        {
            let _ = launcher_security::safe_remove_file_within(&skins_dir, &cape_path);
        }
    }
    tokio::task::spawn_blocking(move || {
        enrich_account(&mut updated, &skins_dir);
        updated
    })
    .await
    .map_err(|e| LauncherErrorPayload::internal(format!("Gagal memproses data jubah akun: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip_accounts() {
        let sample = b"MinecraftLauncherSkinCapeDataTest12345!@#$%^&*()_+";
        let encoded = encode_base64(sample);
        let decoded = decode_base64(&encoded).expect("valid base64");
        assert_eq!(sample.as_slice(), decoded.as_slice());
    }

    #[test]
    fn test_n01a_skin_and_cape_traversal_do_not_write_before_validation() {
        let temp_root = std::env::temp_dir().join(format!("mc-n01a-{}", uuid::Uuid::new_v4()));
        let paths = launcher_platform::DataPaths::new(&temp_root);
        let accounts = launcher_storage::AccountsStore::new(paths.clone());
        let skins_dir = paths.root.join("skins");

        let dummy_b64 = encode_base64(b"PNGDATA");
        let skin_res = update_skin_with_store(
            &accounts,
            &skins_dir,
            UpdateAccountSkinRequest {
                account_id: "../escaped-skin".to_string(),
                skin_data: format!("data:image/png;base64,{}", dummy_b64),
                skin_variant: None,
                cape_data: None,
            },
        );
        assert!(skin_res.is_err());
        assert!(
            !paths.root.join("escaped-skin.png").exists(),
            "N-01a: escaped-skin.png was written outside skins/ before validating account_id!"
        );

        let cape_res = set_custom_cape_with_store(
            &accounts,
            &skins_dir,
            "../escaped-cape",
            &format!("data:image/png;base64,{}", dummy_b64),
        );
        assert!(cape_res.is_err());
        assert!(
            !paths.root.join("escaped-cape_cape.png").exists(),
            "N-01a: escaped-cape_cape.png was written outside skins/ before validating account!"
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_ps01_export_skin_resourcepack_blocks_traversal_and_writes_inside_exports() {
        let temp_root = std::env::temp_dir().join(format!("mc-ps01-{}", uuid::Uuid::new_v4()));
        let paths = launcher_platform::DataPaths::new(&temp_root);
        let accounts = launcher_storage::AccountsStore::new(paths.clone());
        let acc = accounts.create_offline("ExportTester").unwrap();

        for bad_dest in [
            "../../evil.zip",
            "C:\\temp\\evil.zip",
            "sub/pack.zip",
            "CON.zip",
            "not-a-zip.txt",
        ] {
            let res = export_skin_resourcepack_to_dir(&paths, &accounts, &acc.id, bad_dest);
            assert!(res.is_err(), "Destination '{}' must be rejected", bad_dest);
        }

        let ok_path =
            export_skin_resourcepack_to_dir(&paths, &accounts, &acc.id, "my-skin-pack.zip")
                .expect("Valid single .zip filename in exports/ must succeed");
        let ok_pb = std::path::PathBuf::from(&ok_path);
        assert!(ok_pb.exists());
        assert_eq!(ok_pb.parent(), Some(paths.root.join("exports").as_path()));

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_ps02_accounts_rejects_local_path_skin_and_non_png() {
        let temp_root = std::env::temp_dir().join(format!("mc-ps02-{}", uuid::Uuid::new_v4()));
        let paths = launcher_platform::DataPaths::new(&temp_root);
        let accounts = launcher_storage::AccountsStore::new(paths.clone());
        let skins_dir = paths.root.join("skins");
        let acc = accounts.create_offline("SkinTester").unwrap();

        let secret_file = temp_root.join("local-secret.png");
        std::fs::create_dir_all(&temp_root).unwrap();
        std::fs::write(&secret_file, b"\x89PNG\r\n\x1a\nFAKEPNG").unwrap();
        let raw_path_res = update_skin_with_store(
            &accounts,
            &skins_dir,
            UpdateAccountSkinRequest {
                account_id: acc.id.clone(),
                skin_data: secret_file.to_string_lossy().to_string(),
                skin_variant: None,
                cape_data: None,
            },
        );
        assert!(raw_path_res.is_err(), "Raw local path must be rejected");

        let non_png_res = update_skin_with_store(
            &accounts,
            &skins_dir,
            UpdateAccountSkinRequest {
                account_id: acc.id.clone(),
                skin_data: format!("data:image/png;base64,{}", encode_base64(b"NOT_A_PNG")),
                skin_variant: None,
                cape_data: None,
            },
        );
        assert!(non_png_res.is_err(), "Non-PNG bytes must be rejected");

        let valid_png_res = update_skin_with_store(
            &accounts,
            &skins_dir,
            UpdateAccountSkinRequest {
                account_id: acc.id,
                skin_data: format!(
                    "data:image/png;base64,{}",
                    encode_base64(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR")
                ),
                skin_variant: Some("slim".to_string()),
                cape_data: None,
            },
        );
        assert!(valid_png_res.is_ok(), "Valid PNG data URI should succeed");

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
