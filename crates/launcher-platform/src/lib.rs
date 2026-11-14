use launcher_types::MemoryInfo;
use std::path::{Path, PathBuf};

pub mod logs;
pub mod secret_store;
pub use logs::{
    decode_diagnostic_code, sanitize_log_text, strip_ansi, DiagnosticInstanceMeta,
    DiagnosticReportResult, LauncherLogLine, LauncherLogStore,
};
pub use secret_store::SecretStore;

pub const APP_DIR_NAME: &str = "MCLauncher";

#[derive(Debug, Clone)]
pub struct DataPaths {
    pub root: PathBuf,
    pub shared_dir: PathBuf,
    pub versions_dir: PathBuf,
    pub libraries_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub runtimes_dir: PathBuf,
    pub instances_dir: PathBuf,
    pub trash_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub api_cache_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub settings_file: PathBuf,
    pub accounts_file: PathBuf,
    pub secrets_file: PathBuf,
}

impl DataPaths {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root_ref = root.as_ref();
        let root = if root_ref.exists() {
            dunce::canonicalize(root_ref).unwrap_or_else(|_| root_ref.to_path_buf())
        } else {
            root_ref.to_path_buf()
        };
        let shared = root.join("shared");
        let cache = root.join("cache");

        Self {
            shared_dir: shared.clone(),
            versions_dir: shared.join("versions"),
            libraries_dir: shared.join("libraries"),
            assets_dir: shared.join("assets"),
            runtimes_dir: shared.join("runtimes"),
            instances_dir: root.join("instances"),
            trash_dir: root.join(".trash"),
            api_cache_dir: cache.join("api"),
            cache_dir: cache,
            logs_dir: root.join("logs"),
            settings_file: root.join("settings.json"),
            accounts_file: root.join("accounts.json"),
            secrets_file: root.join("secrets.bin"),
            root,
        }
    }

    pub fn default_from_env() -> Self {
        Self::new(resolve_data_root())
    }

    pub fn ensure_root_secure(&self) -> Result<(), launcher_types::LauncherErrorPayload> {
        launcher_security::validate_data_root_candidate(&self.root)?;
        launcher_security::ensure_secure_dir(&self.root, true)
    }
}

/// Menentukan direktori data utama launcher.
/// 1. Cek override env `MCLAUNCHER_DATA_ROOT` (harus lolos validasi keamanan).
/// 2. Windows: `%APPDATA%\MCLauncher`.
/// 3. Linux/Unix: `$XDG_DATA_HOME/MCLauncher` (fallback `~/.local/share/MCLauncher`).
pub fn resolve_data_root() -> PathBuf {
    if let Ok(override_path) = std::env::var("MCLAUNCHER_DATA_ROOT") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if launcher_security::validate_data_root_candidate(&candidate).is_ok() {
                return candidate;
            }
        }
    }

    #[cfg(windows)]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let trimmed = app_data.trim();
            if !trimmed.is_empty() {
                let candidate = PathBuf::from(trimmed).join(APP_DIR_NAME);
                if launcher_security::validate_data_root_candidate(&candidate).is_ok() {
                    return candidate;
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
            let trimmed = xdg_data.trim();
            if !trimmed.is_empty() {
                let candidate = PathBuf::from(trimmed).join(APP_DIR_NAME);
                if launcher_security::validate_data_root_candidate(&candidate).is_ok() {
                    return candidate;
                }
            }
        }
    }

    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            let home_path = PathBuf::from(trimmed);
            #[cfg(windows)]
            let candidate = home_path.join("AppData").join("Roaming").join(APP_DIR_NAME);
            #[cfg(not(windows))]
            let candidate = home_path.join(".local").join("share").join(APP_DIR_NAME);
            if launcher_security::validate_data_root_candidate(&candidate).is_ok() {
                return candidate;
            }
        }
    }

    std::env::temp_dir().join(format!(".{}", APP_DIR_NAME.to_lowercase()))
}

/// Mendapatkan informasi memori sistem fisik aktual.
pub fn get_system_memory() -> MemoryInfo {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };

        unsafe {
            if GlobalMemoryStatusEx(&mut status).is_ok() {
                let total_mb = status.ullTotalPhys / (1024 * 1024);
                let free_mb = status.ullAvailPhys / (1024 * 1024);
                let used_mb = total_mb.saturating_sub(free_mb);
                return MemoryInfo {
                    total_mb,
                    free_mb,
                    used_mb,
                };
            }
        }
    }

    MemoryInfo {
        total_mb: 16384,
        free_mb: 8192,
        used_mb: 8192,
    }
}

/// Mengembalikan memori kerja yang tidak terpakai ke sistem operasi (SEC-04).
pub fn trim_working_set() {
    #[cfg(windows)]
    {
        #[link(name = "psapi")]
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn EmptyWorkingSet(hProcess: *mut std::ffi::c_void) -> i32;
        }
        unsafe {
            let handle = GetCurrentProcess();
            EmptyWorkingSet(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_info() {
        let mem = get_system_memory();
        assert!(mem.total_mb > 0);
        assert!(mem.total_mb >= mem.used_mb);
    }

    #[test]
    fn test_resolve_data_root() {
        let root = resolve_data_root();
        assert!(root.to_string_lossy().contains(APP_DIR_NAME));
    }
}
