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

pub fn run_java_version(executable: &Path) -> Result<String, LauncherErrorPayload> {
    let validated_exe = launcher_security::validate_java_executable_path(executable)?;
    let mut cmd = Command::new(&validated_exe);
    cmd.args(["-XshowSettings:properties", "-version"])
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

    let output = cmd.output().map_err(|e| {
        LauncherErrorPayload::internal(format!("Gagal mengeksekusi biner Java: {}", e))
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
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

    // Cek apakah managed runtime
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

    #[cfg(target_os = "windows")]
    let platform_key = "windows-x64";
    #[cfg(not(target_os = "windows"))]
    let platform_key = "linux";

    let bin_name = if cfg!(windows) { "java.exe" } else { "java" };

    // 1. Managed runtimes di shared/runtimes/<platform>
    let managed_base = paths.runtimes_dir.join(platform_key);
    if let Ok(entries) = std::fs::read_dir(&managed_base) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let java_exe = entry.path().join("bin").join(bin_name);
                if java_exe.exists() {
                    candidates.push(java_exe);
                }
            }
        }
    }

    // 2. JAVA_HOME
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

    // 3. PATH
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

    // Hapus duplikasi berdasarkan canonical path dan hanya terima executable tervalidasi (SEC-03)
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

    // Urutkan: managed lebih dulu, lalu major version tertinggi, lalu 64-bit lebih dulu
    installations.sort_by(|a, b| {
        b.managed
            .cmp(&a.managed)
            .then_with(|| b.major_version.cmp(&a.major_version))
            .then_with(|| b.bits.cmp(&a.bits))
            .then_with(|| a.path.cmp(&b.path))
    });

    installations
}
