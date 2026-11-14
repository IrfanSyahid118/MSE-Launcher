use launcher_types::LauncherErrorPayload;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroize;

#[cfg(target_os = "windows")]
mod dpapi {
    use launcher_types::LauncherErrorPayload;
    use std::ptr::null_mut;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    #[link(name = "kernel32")]
    extern "system" {
        fn CryptProtectData(
            pDataIn: *const DataBlob,
            szDataDescr: *const u16,
            pOptionalEntropy: *const DataBlob,
            pvReserved: *mut std::ffi::c_void,
            pPromptStruct: *mut std::ffi::c_void,
            dwFlags: u32,
            pDataOut: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            pDataIn: *const DataBlob,
            ppszDataDescr: *mut *mut u16,
            pOptionalEntropy: *const DataBlob,
            pvReserved: *mut std::ffi::c_void,
            pPromptStruct: *mut std::ffi::c_void,
            dwFlags: u32,
            pDataOut: *mut DataBlob,
        ) -> i32;

        fn LocalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    pub fn encrypt_bytes(
        data: &[u8],
        _secrets_path: &std::path::Path,
    ) -> Result<Vec<u8>, LauncherErrorPayload> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let in_blob = DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: null_mut(),
        };

        // CRYPTPROTECT_UI_FORBIDDEN (0x1)
        let success = unsafe {
            CryptProtectData(
                &in_blob,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                0x1,
                &mut out_blob,
            )
        };

        if success == 0 {
            return Err(LauncherErrorPayload::internal(
                "DPAPI CryptProtectData gagal.",
            ));
        }

        let slice =
            unsafe { std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize) };
        let result = slice.to_vec();

        unsafe {
            std::ptr::write_bytes(out_blob.pb_data, 0, out_blob.cb_data as usize);
            LocalFree(out_blob.pb_data as *mut std::ffi::c_void);
        }

        Ok(result)
    }

    pub fn decrypt_bytes(
        data: &[u8],
        _secrets_path: &std::path::Path,
    ) -> Result<Vec<u8>, LauncherErrorPayload> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let in_blob = DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: null_mut(),
        };

        let success = unsafe {
            CryptUnprotectData(
                &in_blob,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                0x1,
                &mut out_blob,
            )
        };

        if success == 0 {
            return Err(LauncherErrorPayload::internal(
                "DPAPI CryptUnprotectData gagal.",
            ));
        }

        let slice =
            unsafe { std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize) };
        let result = slice.to_vec();

        unsafe {
            std::ptr::write_bytes(out_blob.pb_data, 0, out_blob.cb_data as usize);
            LocalFree(out_blob.pb_data as *mut std::ffi::c_void);
        }

        Ok(result)
    }
}

#[cfg(not(target_os = "windows"))]
mod dpapi {
    use launcher_types::LauncherErrorPayload;
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    use ring::rand::{SecureRandom, SystemRandom};
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use zeroize::Zeroize;

    const MAGIC_V1: &[u8; 8] = b"MLAESG01";
    const KEY_LEN: usize = 32;
    const NONCE_LEN: usize = 12;

    fn key_path_for(secrets_path: &Path) -> PathBuf {
        let name = secrets_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("secrets.bin");
        secrets_path.with_file_name(format!("{}.key", name))
    }

    fn write_restricted(path: &Path, bytes: &[u8]) -> Result<(), LauncherErrorPayload> {
        if let Some(parent) = path.parent() {
            launcher_security::ensure_secure_dir(parent, true)?;
        }
        if let Ok(meta) = fs::symlink_metadata(path) {
            if meta.file_type().is_symlink() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target berkas kunci secrets tidak boleh berupa symlink.",
                ));
            }
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("key");
        let tmp = path.with_file_name(format!("{}.tmp-{}", name, fastrand::u64(..)));
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let write_res = (|| -> Result<(), LauncherErrorPayload> {
            let mut file = opts.open(&tmp).map_err(|e| {
                LauncherErrorPayload::file_system(format!(
                    "Gagal membuat berkas kunci secrets: {}",
                    e
                ))
            })?;
            file.write_all(bytes).map_err(|e| {
                LauncherErrorPayload::file_system(format!(
                    "Gagal menulis berkas kunci secrets: {}",
                    e
                ))
            })?;
            Ok(())
        })();
        if let Err(e) = write_res {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        fs::rename(&tmp, path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            LauncherErrorPayload::file_system(format!(
                "Gagal menyimpan berkas kunci secrets: {}",
                e
            ))
        })?;
        Ok(())
    }

    fn load_or_create_key(
        secrets_path: &Path,
        create_if_missing: bool,
    ) -> Result<[u8; KEY_LEN], LauncherErrorPayload> {
        let key_path = key_path_for(secrets_path);
        if key_path.exists() {
            let mut raw = fs::read(&key_path).map_err(|e| {
                LauncherErrorPayload::file_system(format!("Gagal membaca kunci secrets: {}", e))
            })?;
            if raw.len() != KEY_LEN {
                raw.zeroize();
                return Err(LauncherErrorPayload::internal(
                    "Panjang kunci secrets tidak valid.",
                ));
            }
            let mut key = [0u8; KEY_LEN];
            key.copy_from_slice(&raw);
            raw.zeroize();
            return Ok(key);
        }

        if !create_if_missing {
            return Err(LauncherErrorPayload::internal(
                "Berkas kunci secrets tidak ditemukan.",
            ));
        }

        let rng = SystemRandom::new();
        let mut key = [0u8; KEY_LEN];
        rng.fill(&mut key).map_err(|_| {
            LauncherErrorPayload::internal("Gagal menghasilkan kunci acak untuk secrets.")
        })?;

        let tmp_path = key_path.with_extension(format!("key.tmp-{}", fastrand::u64(..)));
        write_restricted(&tmp_path, &key)?;
        if let Err(e) = fs::rename(&tmp_path, &key_path) {
            let _ = fs::remove_file(&tmp_path);
            key.zeroize();
            return Err(LauncherErrorPayload::file_system(format!(
                "Gagal menyimpan berkas kunci secrets: {}",
                e
            )));
        }
        Ok(key)
    }

    pub fn encrypt_bytes(
        data: &[u8],
        secrets_path: &Path,
    ) -> Result<Vec<u8>, LauncherErrorPayload> {
        let mut key_bytes = load_or_create_key(secrets_path, true)?;
        let unbound = UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| {
            key_bytes.zeroize();
            LauncherErrorPayload::internal("Gagal menginisialisasi kunci AES-256-GCM.")
        })?;
        key_bytes.zeroize();
        let key = LessSafeKey::new(unbound);

        let rng = SystemRandom::new();
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rng.fill(&mut nonce_bytes).map_err(|_| {
            LauncherErrorPayload::internal("Gagal menghasilkan nonce enkripsi secrets.")
        })?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        let mut in_out = data.to_vec();
        key.seal_in_place_append_tag(nonce, Aad::from(MAGIC_V1), &mut in_out)
            .map_err(|_| LauncherErrorPayload::internal("Gagal mengenkripsi secrets."))?;

        let mut out = Vec::with_capacity(MAGIC_V1.len() + NONCE_LEN + in_out.len());
        out.extend_from_slice(MAGIC_V1);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&in_out);
        Ok(out)
    }

    pub fn decrypt_bytes(
        data: &[u8],
        secrets_path: &Path,
    ) -> Result<Vec<u8>, LauncherErrorPayload> {
        if data.len() < MAGIC_V1.len() + NONCE_LEN || !data.starts_with(MAGIC_V1) {
            return Err(LauncherErrorPayload::internal(
                "Format secrets.bin tidak valid atau korup.",
            ));
        }

        let mut key_bytes = load_or_create_key(secrets_path, false)?;
        let unbound = UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| {
            key_bytes.zeroize();
            LauncherErrorPayload::internal("Gagal menginisialisasi kunci dekripsi secrets.")
        })?;
        key_bytes.zeroize();
        let key = LessSafeKey::new(unbound);

        let nonce_slice = &data[MAGIC_V1.len()..MAGIC_V1.len() + NONCE_LEN];
        let mut nonce_arr = [0u8; NONCE_LEN];
        nonce_arr.copy_from_slice(nonce_slice);
        let nonce = Nonce::assume_unique_for_key(nonce_arr);

        let mut in_out = data[MAGIC_V1.len() + NONCE_LEN..].to_vec();
        let open_res = key
            .open_in_place(nonce, Aad::from(MAGIC_V1), &mut in_out)
            .map(|plain| plain.len());
        let plain_len = match open_res {
            Ok(len) => len,
            Err(_) => {
                in_out.zeroize();
                return Err(LauncherErrorPayload::internal(
                    "Dekripsi atau verifikasi integritas secrets gagal.",
                ));
            }
        };
        in_out.truncate(plain_len);
        Ok(in_out)
    }
}

#[derive(Clone)]
pub struct SecretStore {
    secrets_path: PathBuf,
    cache: Arc<StdRwLock<HashMap<String, String>>>,
    unquarantined_load_error: Arc<StdMutex<Option<String>>>,
}

impl SecretStore {
    pub fn new(secrets_path: PathBuf) -> Self {
        let store = Self {
            secrets_path,
            cache: Arc::new(StdRwLock::new(HashMap::new())),
            unquarantined_load_error: Arc::new(StdMutex::new(None)),
        };
        store.load_from_disk();
        store
    }

    /// SEC-03: Mengetahui apakah backend penyimpanan rahasia menggunakan enkripsi kuat OS (DPAPI / Keyring).
    pub fn is_strong(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn load_from_disk(&self) {
        if !self.secrets_path.exists() {
            return;
        }

        let load_res: Result<HashMap<String, String>, String> = (|| {
            let encrypted_bytes = fs::read(&self.secrets_path)
                .map_err(|e| format!("gagal membaca {}: {}", self.secrets_path.display(), e))?;
            let mut decrypted_bytes = dpapi::decrypt_bytes(&encrypted_bytes, &self.secrets_path)
                .map_err(|e| format!("gagal mendekripsi secrets.bin: {}", e.message))?;
            let parsed = serde_json::from_slice::<HashMap<String, String>>(&decrypted_bytes)
                .map_err(|e| format!("gagal mem-parsing secrets json: {}", e));
            decrypted_bytes.zeroize();
            parsed
        })();

        match load_res {
            Ok(map) => {
                if let Ok(mut cache) = self.cache.try_write() {
                    *cache = map;
                }
            }
            Err(err_msg) => {
                eprintln!("[SecretStore] {}", err_msg);
                match quarantine_corrupt_file(&self.secrets_path) {
                    Ok(backup_path) => {
                        eprintln!(
                            "[SecretStore] File secrets korup dikarantina ke {}",
                            backup_path.display()
                        );
                    }
                    Err(q_err) => {
                        eprintln!("[SecretStore] {}", q_err);
                        if let Ok(mut guard) = self.unquarantined_load_error.lock() {
                            *guard = Some(format!("{}; {}", err_msg, q_err));
                        }
                    }
                }
            }
        }
    }

    fn persist_to_disk(&self, map: &HashMap<String, String>) -> Result<(), LauncherErrorPayload> {
        if let Ok(guard) = self.unquarantined_load_error.lock() {
            if let Some(ref reason) = *guard {
                if self.secrets_path.exists() {
                    return Err(LauncherErrorPayload::file_system(format!(
                        "Menolak menimpa secrets.bin yang gagal dimuat: {}",
                        reason
                    )));
                }
            }
        }

        let mut json_bytes = serde_json::to_vec(map).map_err(|e| {
            LauncherErrorPayload::internal(format!("Gagal encode secrets json: {}", e))
        })?;

        let encrypted = dpapi::encrypt_bytes(&json_bytes, &self.secrets_path);
        json_bytes.zeroize();
        let encrypted = encrypted?;

        if let Some(parent) = self.secrets_path.parent() {
            launcher_security::ensure_secure_dir(parent, true)?;
        }
        if let Ok(meta) = fs::symlink_metadata(&self.secrets_path) {
            if meta.file_type().is_symlink() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target secrets.bin tidak boleh berupa symlink.",
                ));
            }
        }

        let tmp = self
            .secrets_path
            .with_extension(format!("bin.tmp-{}", fastrand::u64(..)));
        {
            use std::io::Write;
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut file = opts.open(&tmp).map_err(|e| {
                LauncherErrorPayload::file_system(format!(
                    "Gagal membuat temporary secrets.bin: {}",
                    e
                ))
            })?;
            if let Err(e) = file.write_all(&encrypted) {
                let _ = fs::remove_file(&tmp);
                return Err(LauncherErrorPayload::file_system(format!(
                    "Gagal menulis temporary secrets.bin: {}",
                    e
                )));
            }
        }

        fs::rename(&tmp, &self.secrets_path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            LauncherErrorPayload::file_system(format!("Gagal menyimpan secrets.bin: {}", e))
        })?;

        Ok(())
    }

    pub fn get_sync(&self, key: &str) -> Option<String> {
        self.cache.read().ok()?.get(key).cloned()
    }

    pub fn set_sync(&self, key: &str, value: &str) -> Result<(), LauncherErrorPayload> {
        let mut cache = self.cache.write().map_err(|_| {
            LauncherErrorPayload::internal("Lock SecretStore mengalami poison error.")
        })?;
        cache.insert(key.to_string(), value.to_string());
        self.persist_to_disk(&cache)
    }

    pub fn delete_sync(&self, key: &str) -> Result<(), LauncherErrorPayload> {
        let mut cache = self.cache.write().map_err(|_| {
            LauncherErrorPayload::internal("Lock SecretStore mengalami poison error.")
        })?;
        if let Some(mut old_val) = cache.remove(key) {
            old_val.zeroize();
            self.persist_to_disk(&cache)?;
        }
        Ok(())
    }

    #[allow(clippy::unused_async)]
    pub async fn get(&self, key: &str) -> Option<String> {
        self.get_sync(key)
    }

    #[allow(clippy::unused_async)]
    pub async fn set(&self, key: &str, value: &str) -> Result<(), LauncherErrorPayload> {
        self.set_sync(key, value)
    }

    #[allow(clippy::unused_async)]
    pub async fn delete(&self, key: &str) -> Result<(), LauncherErrorPayload> {
        self.delete_sync(key)
    }
}

fn quarantine_corrupt_file(path: &std::path::Path) -> Result<PathBuf, String> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("secrets.bin");
    let backup_name = format!("{}.corrupt.{}-{}", file_name, ts, fastrand::u32(..));
    let backup_path = path.with_file_name(backup_name);
    fs::rename(path, &backup_path)
        .map_err(|e| format!("gagal mengkarantina file korup {}: {}", path.display(), e))?;
    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_dpapi_secret_store() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("secrets.bin");
        let store = SecretStore::new(path.clone());

        assert_eq!(store.get("test_key").await, None);

        store
            .set("test_key", "super_secret_value")
            .await
            .expect("set failed");
        assert_eq!(
            store.get("test_key").await,
            Some("super_secret_value".to_string())
        );

        let on_disk = fs::read(&path).expect("read failed");
        assert!(!on_disk.is_empty());
        let as_str = String::from_utf8_lossy(&on_disk);
        assert!(!as_str.contains("super_secret_value"));

        let store2 = SecretStore::new(path);
        assert_eq!(
            store2.get("test_key").await,
            Some("super_secret_value".to_string())
        );

        store2.delete("test_key").await.expect("delete failed");
        assert_eq!(store2.get("test_key").await, None);
    }

    #[test]
    fn test_zeroize_memory_hygiene_bug11() {
        let mut secret_buf = vec![0x41u8; 64];
        assert_eq!(secret_buf[0], 0x41);
        secret_buf.zeroize();
        assert!(secret_buf.iter().all(|&b| b == 0));

        let mut secret_str = "sensitive_refresh_token_xyz".to_string();
        secret_str.zeroize();
        assert!(secret_str.chars().all(|c| c == '\0'));
    }

    #[tokio::test]
    async fn test_n14_corrupt_secrets_file_not_silently_overwritten() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("secrets.bin");
        let corrupt_payload = b"CORRUPT_NOT_DPAPI_OR_JSON_PAYLOAD";
        fs::write(&path, corrupt_payload).unwrap();

        let store = SecretStore::new(path.clone());
        store
            .set("new_key", "new_val")
            .await
            .expect("writing new secret after quarantine should work");

        let backups: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("corrupt"))
            .collect();
        assert!(
            !backups.is_empty(),
            "Corrupt secrets.bin must be quarantined/backed up before any write overwrites it"
        );
        let preserved = fs::read(backups[0].path()).unwrap();
        assert_eq!(preserved, corrupt_payload);
    }
}
