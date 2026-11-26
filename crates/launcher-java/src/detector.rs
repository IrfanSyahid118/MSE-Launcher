use launcher_platform::DataPaths;
use launcher_types::JavaInstallation;
use launcher_types::LauncherErrorPayload;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

static MAJOR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:1\.)?(\d+)").expect("valid regex"));
static VERSION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"version "([^"]+)""#).expect("valid regex"));
static ARCH_MODEL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"sun\.arch\.data\.model\s*=\s*([^\s]+)").expect("valid regex"));
static OS_ARCH_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"os\.arch\s*=\s*([^\s]+)").expect("valid regex"));
static VENDOR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"java\.vendor\s*=\s*([^\r\n]+)").expect("valid regex"));

pub fn java_major(version: &str) -> Option<u32> {
    let caps = MAJOR_REGEX.captures(version)?;
    caps.get(1)?.as_str().parse::<u32>().ok()
}

pub fn resolve_game_executable(java_path: &Path) -> PathBuf {
    if let Some(file_name) = java_path.file_name().and_then(|n| n.to_str()) {
        if file_name.eq_ignore_ascii_case("java.exe") {
            let javaw = java_path.with_file_name("javaw.exe");
            if let Ok(validated) = launcher_security::validate_java_executable_path(&javaw) {
                return validated;
            }
        }
    }
    java_path.to_path_buf()
}

pub fn resolve_probe_executable(executable: &Path) -> Result<PathBuf, LauncherErrorPayload> {
    let validated_exe = launcher_security::validate_java_executable_path(executable)?;
    if validated_exe
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("javaw.exe"))
    {
        let sibling_java = validated_exe.with_file_name("java.exe");
        return launcher_security::validate_java_executable_path(&sibling_java).map_err(|_| {
            LauncherErrorPayload::invalid_input(
                "javaw.exe tidak mendukung probe konsol dan java.exe pendamping tidak ditemukan.",
            )
        });
    }
    Ok(validated_exe)
}

pub fn run_java_version(executable: &Path) -> Result<String, LauncherErrorPayload> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let probe_exe = resolve_probe_executable(executable)?;
    let mut cmd = Command::new(&probe_exe);
    cmd.args(["-XshowSettings:properties", "-version"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        LauncherErrorPayload::internal(format!("Gagal mengeksekusi biner Java: {}", e))
    })?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut out) = stdout_pipe {
            let _ = out.read_to_end(&mut buf);
        }
        buf
    });

    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut err) = stderr_pipe {
            let _ = err.read_to_end(&mut buf);
        }
        buf
    });

    let timeout = Duration::from_secs(10);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(LauncherErrorPayload::internal(
                        "Pemeriksaan versi Java melewati batas waktu (10 detik).",
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(LauncherErrorPayload::internal(format!(
                    "Gagal memantau proses pemeriksaan Java: {}",
                    e
                )));
            }
        }
    }

    let stdout_bytes = stdout_handle.join().unwrap_or_default();
    let stderr_bytes = stderr_handle.join().unwrap_or_default();
    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let combined = format!("{}\n{}", stdout, stderr);

    Ok(combined)
}

pub fn parse_java_output(
    output: &str,
    executable: &Path,
    paths: &DataPaths,
) -> Result<JavaInstallation, LauncherErrorPayload> {
    let version = VERSION_REGEX
        .captures(output)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Gagal mendeteksi string versi Java dari output.")
        })?;

    let major = java_major(&version).ok_or_else(|| {
        LauncherErrorPayload::invalid_input(format!(
            "Gagal mem-parse major version dari '{}'",
            version
        ))
    })?;

    let arch_model = ARCH_MODEL_REGEX
        .captures(output)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str());
    let os_arch = OS_ARCH_REGEX
        .captures(output)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_lowercase());

    let bits = if arch_model == Some("32")
        || os_arch.as_deref() == Some("x86")
        || os_arch.as_deref() == Some("i386")
    {
        32
    } else {
        64
    };

    let vendor = VENDOR_REGEX
        .captures(output)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string());

    let canonical_exe =
        dunce::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
    let canonical_runtimes =
        dunce::canonicalize(&paths.runtimes_dir).unwrap_or_else(|_| paths.runtimes_dir.clone());
    let managed = canonical_exe.starts_with(&canonical_runtimes);

    let resolved_path = resolve_game_executable(&canonical_exe);

    Ok(JavaInstallation {
        path: resolved_path.to_string_lossy().to_string(),
        version,
        major_version: major,
        bits,
        vendor,
        managed,
    })
}

pub fn find_candidate_executables(paths: &DataPaths) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    let platform_key = "windows-arm64";
    #[cfg(all(target_os = "windows", target_arch = "x86"))]
    let platform_key = "windows-x86";
    #[cfg(all(
        target_os = "windows",
        not(any(target_arch = "aarch64", target_arch = "x86"))
    ))]
    let platform_key = "windows-x64";
    #[cfg(all(target_os = "linux", target_arch = "x86"))]
    let platform_key = "linux-i386";
    #[cfg(all(target_os = "linux", not(target_arch = "x86")))]
    let platform_key = "linux";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let platform_key = "mac-os-arm64";
    #[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
    let platform_key = "mac-os";
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let platform_key = "linux";

    let bin_name = if cfg!(windows) { "java.exe" } else { "java" };

    let managed_base = paths.runtimes_dir.join(platform_key);
    if let Ok(entries) = std::fs::read_dir(&managed_base) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                let bundle_exe = entry
                    .path()
                    .join("jre.bundle")
                    .join("Contents")
                    .join("Home")
                    .join("bin")
                    .join(bin_name);
                if bundle_exe.exists() {
                    candidates.push(bundle_exe);
                } else {
                    let java_exe = entry.path().join("bin").join(bin_name);
                    if java_exe.exists() {
                        candidates.push(java_exe);
                    }
                }
            }
        }
    }

    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(java_home);
        let exe = p.join("bin").join(bin_name);
        if exe.exists() {
            candidates.push(exe);
        } else {
            let root_exe = p.join(bin_name);
            if root_exe.exists() {
                candidates.push(root_exe);
            }
        }
    }

    let path_sep = if cfg!(windows) { ';' } else { ':' };
    if let Ok(path_var) = std::env::var("PATH") {
        for entry in path_var.split(path_sep) {
            let trimmed = entry.trim().trim_matches('"');
            if trimmed.is_empty() {
                continue;
            }
            let p = PathBuf::from(trimmed);
            let exe = if p.to_string_lossy().to_lowercase().ends_with(bin_name) {
                p
            } else {
                p.join(bin_name)
            };
            if exe.exists() {
                candidates.push(exe);
            }
        }
    }

    #[cfg(not(windows))]
    {
        for dir in &["/usr/lib/jvm", "/usr/java", "/opt/java"] {
            let path = PathBuf::from(dir);
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let exe = entry.path().join("bin").join("java");
                    if exe.exists() {
                        candidates.push(exe);
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    {
        let program_roots = [
            std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string()),
            std::env::var("ProgramFiles(x86)")
                .unwrap_or_else(|_| "C:\\Program Files (x86)".to_string()),
            std::env::var("ProgramW6432").unwrap_or_else(|_| "C:\\Program Files".to_string()),
        ];

        let vendors = [
            "Java",
            "Eclipse Adoptium",
            "BellSoft",
            "Microsoft",
            "Amazon Corretto",
            "Zulu",
            "Semeru",
        ];

        for root_str in &program_roots {
            let root = Path::new(root_str);
            for vendor in &vendors {
                let vendor_dir = root.join(vendor);
                if let Ok(entries) = std::fs::read_dir(&vendor_dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            let exe = entry.path().join("bin").join("java.exe");
                            if exe.exists() {
                                candidates.push(exe);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut unique = std::collections::HashSet::new();
    let mut result = Vec::new();

    for c in candidates {
        let Ok(canonical) = launcher_security::validate_java_executable_path(&c) else {
            continue;
        };
        let key = canonical.to_string_lossy().to_lowercase();
        if unique.insert(key) {
            result.push(canonical);
        }
    }

    result
}

pub fn detect_all_installations(paths: &DataPaths) -> Vec<JavaInstallation> {
    let candidates = find_candidate_executables(paths);
    let mut installations = Vec::new();

    for exe in candidates {
        if let Ok(output) = run_java_version(&exe) {
            if let Ok(inst) = parse_java_output(&output, &exe, paths) {
                installations.push(inst);
            }
        }
    }

    installations.sort_by(|a, b| {
        b.managed
            .cmp(&a.managed)
            .then_with(|| b.major_version.cmp(&a.major_version))
            .then_with(|| b.bits.cmp(&a.bits))
            .then_with(|| a.path.cmp(&b.path))
    });

    installations
}

#[cfg(test)]
mod tests {
    // Only used by the Windows-only test below; without this the import
    // is unused (and CI clippy runs with -D warnings) on Linux/macOS.
    #[cfg(windows)]
    use super::*;

    // javaw.exe -> java.exe is Windows-only behaviour: on non-Windows the
    // security validator only accepts an executable named "java", so this
    // test can never pass on Linux/macOS.
    #[test]
    #[cfg(windows)]
    fn test_resolve_probe_executable_switches_javaw_to_java() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("mcl-java-probe-{}-{}", std::process::id(), nanos));
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let java_exe = bin.join("java.exe");
        let javaw_exe = bin.join("javaw.exe");
        std::fs::write(&java_exe, b"stub").unwrap();
        std::fs::write(&javaw_exe, b"stub").unwrap();

        let resolved = resolve_probe_executable(&javaw_exe).expect("should switch to java.exe");
        assert_eq!(
            resolved.file_name().and_then(|s| s.to_str()),
            Some("java.exe")
        );

        std::fs::remove_file(&java_exe).unwrap();
        assert!(resolve_probe_executable(&javaw_exe).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
