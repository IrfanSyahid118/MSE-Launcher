use chrono::Utc;
use md5::{Digest, Md5};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::fs_atomic::{read_json_file, write_json_atomic};
use launcher_platform::DataPaths;
use launcher_types::{Account, AccountType, AccountsFile, LauncherErrorPayload};

/// Menghitung UUID offline Minecraft standar Java Edition.
/// Format: MD5("OfflinePlayer:" + username) dengan versi 3 (0x30) dan varian RFC 4122 (0x80).
pub fn offline_uuid(username: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(b"OfflinePlayer:");
    hasher.update(username.as_bytes());
    let mut hash = hasher.finalize();

    if let Some(b) = hash.get_mut(6) {
        *b = (*b & 0x0f) | 0x30;
    }
    if let Some(b) = hash.get_mut(8) {
        *b = (*b & 0x3f) | 0x80;
    }

    Uuid::from_bytes(hash.into()).to_string()
}

/// Menghitung UUID offline Minecraft standar dengan penyesuaian model lengan (slim vs classic).
/// Di Minecraft client, (uuid.hashCode() & 1) menentukan model: 1 = slim (Alex 3px), 0 = classic (Steve 4px).
pub fn offline_uuid_with_variant(username: &str, is_slim: bool) -> String {
    let mut hasher = Md5::new();
    hasher.update(b"OfflinePlayer:");
    hasher.update(username.as_bytes());
    let mut hash = hasher.finalize();

    if let Some(b) = hash.get_mut(6) {
        *b = (*b & 0x0f) | 0x30;
    }
    if let Some(b) = hash.get_mut(8) {
        *b = (*b & 0x3f) | 0x80;
    }

    let mut bytes: [u8; 16] = hash.into();
    let mut most_arr = [0u8; 8];
    let mut least_arr = [0u8; 8];
    most_arr.copy_from_slice(&bytes[..8]);
    least_arr.copy_from_slice(&bytes[8..]);
    let most = u64::from_be_bytes(most_arr);
    let least = u64::from_be_bytes(least_arr);
    let hilo = most ^ least;
    let hi = (hilo >> 32) as u32;
    let lo = hilo as u32;
    let current_bit = (hi ^ lo) & 1;

    let target_bit = if is_slim { 1 } else { 0 };
    if current_bit != target_bit {
        bytes[15] ^= 1;
    }

    Uuid::from_bytes(bytes).to_string()
}

#[derive(Clone)]
pub struct AccountsStore {
    paths: DataPaths,
    cache: Arc<RwLock<AccountsFile>>,
    unrecovered_load_error: Arc<RwLock<Option<String>>>,
}

impl AccountsStore {
    pub fn new(paths: DataPaths) -> Self {
        let (initial, load_err) = Self::load_from_disk(&paths);
        Self {
            paths,
            cache: Arc::new(RwLock::new(initial)),
            unrecovered_load_error: Arc::new(RwLock::new(load_err)),
        }
    }

    fn load_from_disk(paths: &DataPaths) -> (AccountsFile, Option<String>) {
        match read_json_file::<AccountsFile>(&paths.accounts_file) {
            Ok(Some(a)) => (a, None),
            Ok(None) => {
                let default_file = AccountsFile::default();
                let _ = write_json_atomic(&paths.accounts_file, &default_file);
                (default_file, None)
            }
            Err(e) => {
                tracing::error!(
                    "Gagal membaca berkas akun {:?}; memakai state default di memori tanpa menimpa disk: {:?}",
                    paths.accounts_file,
                    e
                );
                (AccountsFile::default(), Some(e.message))
            }
        }
    }

    fn ensure_writable(&self) -> Result<(), LauncherErrorPayload> {
        if let Some(reason) = self
            .unrecovered_load_error
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            if self.paths.accounts_file.exists() {
                return Err(LauncherErrorPayload::file_system(format!(
                    "Menolak menimpa accounts.json yang gagal dimuat: {}",
                    reason
                )));
            }
        }
        Ok(())
    }

    fn persist_next(
        &self,
        cache_guard: &mut AccountsFile,
        next: AccountsFile,
    ) -> Result<(), LauncherErrorPayload> {
        self.ensure_writable()?;
        write_json_atomic(&self.paths.accounts_file, &next)?;
        *self
            .unrecovered_load_error
            .write()
            .unwrap_or_else(|e| e.into_inner()) = None;
        *cache_guard = next;
        Ok(())
    }

    pub fn list(&self) -> Vec<Account> {
        self.cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .accounts
            .clone()
    }

    pub fn get(&self, id: &str) -> Option<Account> {
        self.cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .accounts
            .iter()
            .find(|a| a.id == id || a.uuid == id)
            .cloned()
    }

    pub fn create_offline(&self, raw_username: &str) -> Result<Account, LauncherErrorPayload> {
        let username = raw_username.trim();
        if username.is_empty() {
            return Err(LauncherErrorPayload::invalid_input(
                "Username tidak boleh kosong.",
            ));
        }

        if username.len() > 24 {
            return Err(LauncherErrorPayload::invalid_input(
                "Username terlalu panjang (maksimal 24 karakter).",
            ));
        }

        if !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(LauncherErrorPayload::invalid_input(
                "Username hanya boleh mengandung huruf, angka, '_' atau '-'.",
            ));
        }

        let uuid = offline_uuid(username);

        let mut cache_guard = self.cache.write().unwrap_or_else(|e| e.into_inner());

        if let Some(existing) = cache_guard.accounts.iter().find(|a| {
            a.username.eq_ignore_ascii_case(username) && a.account_type == AccountType::Offline
        }) {
            return Ok(existing.clone());
        }

        let new_account = Account {
            id: uuid.clone(),
            username: username.to_string(),
            uuid,
            account_type: AccountType::Offline,
            created_at: Utc::now().to_rfc3339(),
            expires_at: None,
            tokens: None,
            xuid: None,
            skin_url: None,
            skin_variant: None,
            cape_url: None,
            active_cape_id: None,
            capes: Vec::new(),
        };

        let mut next = cache_guard.clone();
        next.accounts.push(new_account.clone());
        self.persist_next(&mut cache_guard, next)?;

        Ok(new_account)
    }

    pub fn save_account(&self, account: Account) -> Result<(), LauncherErrorPayload> {
        let mut cache_guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
        let mut next = cache_guard.clone();
        if let Some(existing) = next.accounts.iter_mut().find(|a| a.id == account.id) {
            *existing = account;
        } else {
            next.accounts.push(account);
        }
        self.persist_next(&mut cache_guard, next)
    }

    pub fn update_skin(
        &self,
        account_id: &str,
        skin_url: Option<String>,
        skin_variant: Option<String>,
        cape_url: Option<String>,
    ) -> Result<Account, LauncherErrorPayload> {
        let mut cache_guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
        let mut next = cache_guard.clone();
        let acc = next
            .accounts
            .iter_mut()
            .find(|a| a.id == account_id || a.uuid == account_id)
            .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;

        if skin_url.is_some() {
            acc.skin_url = skin_url;
        }
        if let Some(ref variant) = skin_variant {
            acc.skin_variant = Some(variant.clone());
            if acc.account_type == AccountType::Offline {
                let is_slim = variant.eq_ignore_ascii_case("slim");
                acc.uuid = offline_uuid_with_variant(&acc.username, is_slim);
            }
        }
        if cape_url.is_some() {
            acc.cape_url = cape_url;
        }

        let updated = acc.clone();
        self.persist_next(&mut cache_guard, next)?;
        Ok(updated)
    }

    pub fn update_cape(
        &self,
        account_id: &str,
        cape_url: Option<String>,
        active_cape_id: Option<String>,
    ) -> Result<Account, LauncherErrorPayload> {
        let mut cache_guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
        let mut next = cache_guard.clone();
        let acc = next
            .accounts
            .iter_mut()
            .find(|a| a.id == account_id || a.uuid == account_id)
            .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;

        acc.cape_url = cape_url;
        acc.active_cape_id = active_cape_id.clone();
        for cape in &mut acc.capes {
            if Some(&cape.id) == active_cape_id.as_ref() {
                cape.state = "ACTIVE".to_string();
            } else {
                cape.state = "INACTIVE".to_string();
            }
        }

        let updated = acc.clone();
        self.persist_next(&mut cache_guard, next)?;
        Ok(updated)
    }

    pub fn remove(&self, id: &str) -> Result<(), LauncherErrorPayload> {
        let mut cache_guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
        let mut next = cache_guard.clone();
        next.accounts.retain(|a| a.id != id && a.uuid != id);
        self.persist_next(&mut cache_guard, next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offline_uuid_steve() {
        let uuid = offline_uuid("Steve");
        assert_eq!(uuid, "5627dd98-e6be-3c21-b8a8-e92344183641");
    }

    #[test]
    fn test_offline_uuid_alex() {
        let uuid = offline_uuid("Alex");
        assert_eq!(uuid, "36532b5e-c442-3dbb-a24c-c7e55d0f979a");
    }

    #[test]
    fn test_offline_uuid_with_variant() {
        let slim_uuid = offline_uuid_with_variant("Player", true);
        let u_slim = Uuid::parse_str(&slim_uuid).unwrap();
        let bytes_slim = u_slim.as_bytes();
        let most_slim = u64::from_be_bytes(bytes_slim[0..8].try_into().unwrap());
        let least_slim = u64::from_be_bytes(bytes_slim[8..16].try_into().unwrap());
        let hilo_slim = most_slim ^ least_slim;
        let hash_slim = ((hilo_slim >> 32) as u32) ^ (hilo_slim as u32);
        assert_eq!(hash_slim & 1, 1);

        let classic_uuid = offline_uuid_with_variant("Player", false);
        let u_classic = Uuid::parse_str(&classic_uuid).unwrap();
        let bytes_classic = u_classic.as_bytes();
        let most_classic = u64::from_be_bytes(bytes_classic[0..8].try_into().unwrap());
        let least_classic = u64::from_be_bytes(bytes_classic[8..16].try_into().unwrap());
        let hilo_classic = most_classic ^ least_classic;
        let hash_classic = ((hilo_classic >> 32) as u32) ^ (hilo_classic as u32);
        assert_eq!(hash_classic & 1, 0);
    }

    #[test]
    fn test_n16_offline_skin_variant_change_keeps_id_in_sync() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(tmp.path());
        let store = AccountsStore::new(paths.clone());

        let created = store.create_offline("VariantSyncUser").unwrap();
        let old_id = created.id.clone();

        let skins_dir = paths.root.join("skins");
        std::fs::create_dir_all(&skins_dir).unwrap();
        let old_skin_file = skins_dir.join(format!("{}.png", old_id));
        std::fs::write(&old_skin_file, b"PNG_DATA").unwrap();

        let target_variant = if offline_uuid_with_variant("VariantSyncUser", true) != created.uuid {
            "slim"
        } else {
            "classic"
        };

        let updated = store
            .update_skin(
                &old_id,
                Some(old_skin_file.to_string_lossy().to_string()),
                Some(target_variant.to_string()),
                None,
            )
            .unwrap();

        let expected_uuid = offline_uuid_with_variant("VariantSyncUser", target_variant == "slim");
        assert_eq!(
            updated.id, old_id,
            "Account ID must stay stable when offline variant changes"
        );
        assert_eq!(
            updated.uuid, expected_uuid,
            "Game launch UUID must reflect chosen skin variant"
        );
        assert!(
            store.get(&old_id).is_some(),
            "Account lookup by previous active_account_id ({}) failed after skin_variant={} changed id to {}",
            old_id,
            target_variant,
            updated.id
        );
        assert!(
            skins_dir.join(format!("{}.png", updated.id)).exists(),
            "Expected skin file for account.id ({}.png) to exist, only {}.png exists",
            updated.id,
            old_id
        );
    }
}
