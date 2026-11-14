use launcher_platform::DataPaths;
use launcher_types::LauncherErrorPayload;
use launcher_version::{is_library_allowed, MojangVersionDetail, VersionService};
use std::path::{Path, PathBuf};

pub fn extract_natives(
    detail: &MojangVersionDetail,
    paths: &DataPaths,
    instance_dir: &Path,
) -> Result<PathBuf, LauncherErrorPayload> {
    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "osx"
    };
    let current_arch = if cfg!(target_arch = "x86_64") {
        "64"
    } else if cfg!(target_arch = "x86") {
        "32"
    } else {
        "arm64"
    };
    let default_native_key = format!("natives-{}", current_os);

    let launch_nonce = uuid::Uuid::new_v4().simple().to_string();
    let natives_dir = instance_dir.join(format!("natives-{}", launch_nonce));
    launcher_security::ensure_dir_within(instance_dir, &natives_dir, true)?;

    for lib in &detail.libraries {
        if !is_library_allowed(
            lib.rules.as_deref(),
            current_os,
            if current_arch == "64" {
                "x64"
            } else {
                current_arch
            },
        ) {
            continue;
        }

        if let Some(ref downloads) = lib.downloads {
            if let Some(ref classifiers) = downloads.classifiers {
                let native_key = lib
                    .natives
                    .as_ref()
                    .and_then(|n| n.get(current_os))
                    .map(|s| s.replace("${arch}", current_arch))
                    .unwrap_or_else(|| default_native_key.clone());

                if let Some(artifact) = classifiers.get(&native_key) {
                    let rel_path = artifact.path.clone().unwrap_or_else(|| {
                        VersionService::maven_to_path(&format!("{}:{}", lib.name, native_key))
                    });

                    let jar_path =
                        launcher_security::safe_subpath(&paths.libraries_dir, &rel_path)?;
                    if jar_path.exists() {
                        launcher_archive::extract_zip(
                            &jar_path,
                            &natives_dir,
                            &["META-INF/", ".git/"],
                        )?;
                    }
                }
            }
        }
    }

    Ok(natives_dir)
}
