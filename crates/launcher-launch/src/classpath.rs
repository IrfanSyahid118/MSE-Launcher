use launcher_platform::DataPaths;
use launcher_version::{is_library_allowed, MojangVersionDetail, VersionService};
use std::path::PathBuf;

pub fn assemble_classpath(detail: &MojangVersionDetail, paths: &DataPaths) -> Vec<PathBuf> {
    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "osx"
    };
    let current_arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "arm64"
    };

    let mut classpath = Vec::new();

    // Place loader and dependency JARs before the client JAR so class patching works.
    for lib in &detail.libraries {
        if !is_library_allowed(lib.rules.as_deref(), current_os, current_arch) {
            continue;
        }

        if let Some(ref downloads) = lib.downloads {
            if let Some(ref artifact) = downloads.artifact {
                let rel_path = artifact
                    .path
                    .clone()
                    .unwrap_or_else(|| VersionService::maven_to_path(&lib.name));

                if let Ok(jar_path) =
                    launcher_security::safe_subpath(&paths.libraries_dir, &rel_path)
                {
                    if jar_path.exists() {
                        classpath.push(jar_path);
                    }
                }
            }
        } else {
            let rel_path = VersionService::maven_to_path(&lib.name);
            if let Ok(jar_path) = launcher_security::safe_subpath(&paths.libraries_dir, &rel_path) {
                if jar_path.exists() {
                    classpath.push(jar_path);
                }
            }
        }
    }

    if let Ok(ver_dir) = launcher_security::safe_subpath(&paths.versions_dir, &detail.id) {
        if let Ok(client_jar) =
            launcher_security::safe_subpath(&ver_dir, &format!("{}.jar", detail.id))
        {
            if client_jar.exists() {
                classpath.push(client_jar);
                let mut unique = std::collections::HashSet::new();
                classpath.retain(|p| unique.insert(p.clone()));
                return classpath;
            }
        }
    }
    if let Some(ref inherits_from) = detail.inherits_from {
        if let Ok(parent_dir) = launcher_security::safe_subpath(&paths.versions_dir, inherits_from)
        {
            if let Ok(parent_jar) =
                launcher_security::safe_subpath(&parent_dir, &format!("{}.jar", inherits_from))
            {
                if parent_jar.exists() {
                    classpath.push(parent_jar);
                }
            }
        }
    }

    let mut unique = std::collections::HashSet::new();
    classpath.retain(|p| unique.insert(p.clone()));

    classpath
}
