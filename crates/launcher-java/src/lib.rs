pub mod detector;
pub mod manager;

pub use detector::*;
pub use manager::*;

use launcher_platform::DataPaths;
use launcher_types::{JavaInstallation, LauncherErrorPayload};
use std::path::Path;

#[derive(Clone)]
pub struct JavaService {
    paths: DataPaths,
}

impl JavaService {
    pub fn new(paths: DataPaths) -> Self {
        Self { paths }
    }

    pub fn detect(&self) -> Vec<JavaInstallation> {
        detector::detect_all_installations(&self.paths)
    }

    pub fn validate(&self, path: &str) -> Result<JavaInstallation, LauncherErrorPayload> {
        let p = Path::new(path);
        let validated_exe = launcher_security::validate_java_executable_path(p)?;
        let output = detector::run_java_version(&validated_exe)?;
        detector::parse_java_output(&output, &validated_exe, &self.paths)
    }

    pub async fn ensure<F>(
        &self,
        major: u32,
        component: Option<&str>,
        downloader: &launcher_download::DownloadManager,
        progress_callback: F,
        cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<JavaInstallation, LauncherErrorPayload>
    where
        F: Fn(launcher_download::DownloadProgress) + Send + Sync + 'static,
    {
        let http = launcher_net::HttpClient::default();
        manager::ensure_managed_java(
            &self.paths,
            major,
            component,
            &http,
            downloader,
            progress_callback,
            cancelled,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_major_parsing() {
        assert_eq!(java_major("1.8.0_51"), Some(8));
        assert_eq!(java_major("17.0.8"), Some(17));
        assert_eq!(java_major("21.0.1"), Some(21));
        assert_eq!(java_major("25.0.1"), Some(25));
    }

    #[test]
    fn test_detect_real() {
        let paths = DataPaths::default_from_env();
        let service = JavaService::new(paths);
        let list = service.detect();
        for item in &list {
            println!(
                "Detected Java: {} (major {}) at {}",
                item.version, item.major_version, item.path
            );
        }
    }

    #[test]
    fn test_n01d_java_manifest_traversal_rejected() {
        let target_dir = std::path::PathBuf::from("C:\\fake\\runtime");
        assert!(manager::resolve_manifest_rel_path(&target_dir, "../../evil.exe").is_err());
        assert!(manager::resolve_manifest_rel_path(&target_dir, "/etc/passwd").is_err());
        assert!(manager::resolve_manifest_rel_path(&target_dir, "bin/java.exe").is_ok());
    }

    #[test]
    fn test_ps03_relative_symlink_target_validation() {
        let temp = tempfile::tempdir().unwrap();
        let runtime_root = temp.path().join("jre-21");
        let link_in_lib = runtime_root.join("lib").join("jli.so");
        assert!(launcher_security::validate_relative_symlink_target_within(
            &runtime_root,
            &link_in_lib,
            "../lib/jli/libjli.so",
        )
        .is_ok());
        assert!(launcher_security::validate_relative_symlink_target_within(
            &runtime_root,
            &link_in_lib,
            "../../../outside/secret",
        )
        .is_err());
        assert!(launcher_security::validate_relative_symlink_target_within(
            &runtime_root,
            &link_in_lib,
            "/etc/passwd",
        )
        .is_err());
    }

    #[test]
    fn test_ps07_java_validate_rejects_non_java_executable() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(temp.path());
        let service = JavaService::new(paths);
        let not_java = temp.path().join("powershell.bat");
        std::fs::write(&not_java, b"@echo off\r\n").unwrap();
        assert!(service.validate(not_java.to_str().unwrap()).is_err());
    }
}
