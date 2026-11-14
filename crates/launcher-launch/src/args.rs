use launcher_types::{JvmPreset, LauncherErrorPayload};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const ARGFILE_THRESHOLD: usize = 2048;

static VAR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{([^}]+)\}").expect("valid regex"));
static TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|\S+"#).expect("valid regex")
});

const FORBIDDEN_JVM_PREFIXES: &[&str] = &[
    "-xmx",
    "-xms",
    "-cp",
    "-classpath",
    "--class-path",
    "-djava.library.path",
    "-djava.system.class.loader",
    "-djava.security.properties",
    "-djavax.net.ssl.truststore",
    "-javaagent",
    "-agentlib",
    "-agentpath",
    "-xbootclasspath",
    "-xlog",
    "-xx:onerror",
    "-xx:onoutofmemoryerror",
    "-xx:errorfile",
    "-xx:heapdumppath",
    "-xx:logfile",
    "-djava.security.manager",
    "--patch-module",
    "--add-modules",
    "-xdebug",
    "-xrunjdwp",
    "-xx:flags",
    "-xx:vmoptionsfile",
];

pub fn replace_variables(template: &str, variables: &HashMap<&str, &str>) -> String {
    VAR_REGEX
        .replace_all(template, |caps: &regex::Captures| {
            let key = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            variables.get(key).copied().unwrap_or("")
        })
        .to_string()
}

pub fn is_allowed_jvm_arg(clean: &str) -> bool {
    // Layer 1: Strict allowlist check
    let is_allowed_pattern = if let Some(stripped) = clean.strip_prefix("-XX:") {
        stripped
            .chars()
            .next()
            .is_some_and(|c| c == '+' || c == '-' || c.is_ascii_alphanumeric())
    } else if let Some(stripped) = clean.strip_prefix("-D") {
        stripped
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '.')
    } else if let Some(stripped) = clean.strip_prefix("-X") {
        if stripped.starts_with("X:") {
            false
        } else {
            stripped
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric())
        }
    } else {
        clean.starts_with("--add-opens=")
            || clean.starts_with("--add-exports=")
            || clean.starts_with("--illegal-access=")
            || clean == "--enable-preview"
    };

    if !is_allowed_pattern {
        return false;
    }

    // Layer 2: Defense-in-depth blocklist check
    if clean.starts_with('@') {
        return false;
    }

    let lower = clean.to_lowercase();
    let is_forbidden = FORBIDDEN_JVM_PREFIXES.iter().any(|prefix| {
        lower == *prefix
            || lower.starts_with(prefix)
            || lower.starts_with(&format!("{}=", prefix))
            || lower.starts_with(&format!("{}:", prefix))
    });

    !is_forbidden
}

pub fn get_preset_jvm_args(preset: JvmPreset, custom_args: &str) -> Vec<String> {
    match preset {
        JvmPreset::Performance => vec![
            "-XX:+UseG1GC".to_string(),
            "-XX:+ParallelRefProcEnabled".to_string(),
            "-XX:MaxGCPauseMillis=200".to_string(),
            "-XX:+UnlockExperimentalVMOptions".to_string(),
            "-XX:+DisableExplicitGC".to_string(),
            "-XX:+AlwaysPreTouch".to_string(),
        ],
        JvmPreset::LowMemory => vec![
            "-XX:+UseSerialGC".to_string(),
            "-XX:MinHeapFreeRatio=5".to_string(),
            "-XX:MaxHeapFreeRatio=10".to_string(),
        ],
        JvmPreset::Custom => {
            if custom_args.trim().is_empty() {
                return Vec::new();
            }

            let mut sanitized = Vec::new();

            for cap in TOKEN_REGEX.find_iter(custom_args) {
                let token = cap.as_str();
                let clean = token
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .or_else(|| token.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                    .unwrap_or(token);

                if is_allowed_jvm_arg(clean) {
                    sanitized.push(clean.to_string());
                }
            }

            sanitized
        }
        JvmPreset::Default => Vec::new(),
    }
}

pub fn expand_arguments_json(
    raw_args: &serde_json::Value,
    variables: &HashMap<&str, &str>,
    current_os: &str,
    current_arch: &str,
) -> Vec<String> {
    let mut output = Vec::new();

    let items = match raw_args.as_array() {
        Some(arr) => arr,
        None => return output,
    };

    for item in items {
        if let Some(s) = item.as_str() {
            output.push(replace_variables(s, variables));
            continue;
        }

        if let Some(obj) = item.as_object() {
            // Cek rules jika ada
            if let Some(rules_val) = obj.get("rules").and_then(|r| r.as_array()) {
                let rules: Vec<launcher_version::MojangRule> =
                    serde_json::from_value(serde_json::Value::Array(rules_val.clone()))
                        .unwrap_or_default();
                if !launcher_version::is_library_allowed(Some(&rules), current_os, current_arch) {
                    continue;
                }
            }

            if let Some(value) = obj.get("value") {
                if let Some(val_str) = value.as_str() {
                    output.push(replace_variables(val_str, variables));
                } else if let Some(val_arr) = value.as_array() {
                    for v in val_arr {
                        if let Some(vs) = v.as_str() {
                            output.push(replace_variables(vs, variables));
                        }
                    }
                }
            }
        }
    }

    output
}

pub fn argfile_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn should_use_argfile(java_major: u32, total_len: usize, has_access_token: bool) -> bool {
    if java_major < 9 {
        return false;
    }
    has_access_token || total_len > ARGFILE_THRESHOLD
}

pub fn clamp_heap_memory(raw_min: u32, raw_max: u32) -> (u32, u32) {
    let min_mb = raw_min.min(raw_max).max(256);
    let max_mb = raw_max.max(min_mb);
    (min_mb, max_mb)
}

pub fn create_argfile(args: &[String], target_dir: &Path) -> Result<PathBuf, LauncherErrorPayload> {
    use std::io::Write;
    let argfile_path = target_dir.join(format!("args-{}.txt", uuid::Uuid::new_v4().simple()));
    launcher_security::ensure_descendants_no_symlink(target_dir, &argfile_path)?;
    let content = args
        .iter()
        .map(|a| argfile_quote(a))
        .collect::<Vec<_>>()
        .join("\n");

    let mut file = launcher_security::safe_create_new_file(&argfile_path, true)?;
    if let Err(e) = file.write_all(content.as_bytes()) {
        let _ = fs::remove_file(&argfile_path);
        return Err(LauncherErrorPayload::file_system(format!(
            "Gagal menulis argfile: {}",
            e
        )));
    }

    Ok(argfile_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_replacement() {
        let mut vars = HashMap::new();
        vars.insert("username", "Steve");
        vars.insert("version", "1.20.4");

        let res = replace_variables("--username ${username} --version ${version}", &vars);
        assert_eq!(res, "--username Steve --version 1.20.4");
    }

    #[test]
    fn test_forbidden_jvm_args() {
        let custom = "-XX:+UseG1GC -Xmx8G -cp hacked.jar -Djava.library.path=bad -Djava.system.class.loader=Evil -Djava.security.properties=evil.props -Djavax.net.ssl.trustStore=evil.jks -Xlog:all=trace:file=evil.log -XX:HeapDumpPath=dump.hprof --add-modules=ALL-SYSTEM -Dmyprop=true @injected_argfile.txt -XX:Flags=options.txt -XX:VMOptionsFile=evil.vmoptions arbitrary_script.sh --malicious-flag --add-opens=java.base/java.lang=ALL-UNNAMED";
        let filtered = get_preset_jvm_args(JvmPreset::Custom, custom);
        // Allowlist accepted
        assert!(filtered.contains(&"-XX:+UseG1GC".to_string()));
        assert!(filtered.contains(&"-Dmyprop=true".to_string()));
        assert!(filtered.contains(&"--add-opens=java.base/java.lang=ALL-UNNAMED".to_string()));
        // Allowlist rejected non-matching tokens
        assert!(!filtered.iter().any(|a| a == "arbitrary_script.sh"));
        assert!(!filtered.iter().any(|a| a == "--malicious-flag"));
        assert!(!filtered.iter().any(|a| a.starts_with("--add-modules")));
        // Blocklist layer rejected
        assert!(!filtered.iter().any(|a| a.starts_with("-Xmx")));
        assert!(!filtered.iter().any(|a| a.starts_with("-cp")));
        assert!(!filtered
            .iter()
            .any(|a| a.starts_with("-Djava.library.path")));
        assert!(!filtered
            .iter()
            .any(|a| a.starts_with("-Djava.system.class.loader")));
        assert!(!filtered
            .iter()
            .any(|a| a.starts_with("-Djava.security.properties")));
        assert!(!filtered
            .iter()
            .any(|a| a.to_lowercase().starts_with("-djavax.net.ssl.truststore")));
        assert!(!filtered
            .iter()
            .any(|a| a.to_lowercase().starts_with("-xlog")));
        assert!(!filtered
            .iter()
            .any(|a| a.to_lowercase().starts_with("-xx:heapdumppath")));
        assert!(!filtered.iter().any(|a| a.starts_with('@')));
        assert!(!filtered
            .iter()
            .any(|a| a.to_lowercase().starts_with("-xx:flags")));
        assert!(!filtered
            .iter()
            .any(|a| a.to_lowercase().starts_with("-xx:vmoptionsfile")));
    }

    #[test]
    fn test_should_use_argfile() {
        // Java 8 tidak boleh argfile
        assert!(!should_use_argfile(8, 100, true));
        assert!(!should_use_argfile(8, 5000, true));

        // Java 17 selalu argfile jika has_access_token = true
        assert!(should_use_argfile(17, 100, true));
        assert!(!should_use_argfile(17, 100, false));

        // Java 17 argfile jika panjang > 2048 chars meski tanpa token
        assert!(should_use_argfile(17, 3000, false));
    }

    #[test]
    fn test_clamp_heap_memory_bug17() {
        assert_eq!(clamp_heap_memory(1024, 4096), (1024, 4096));
        assert_eq!(clamp_heap_memory(8192, 2048), (2048, 2048));
        assert_eq!(clamp_heap_memory(64, 128), (256, 256));
    }

    #[test]
    fn test_create_and_sweep_argfile_bug08() {
        let temp = tempfile::tempdir().unwrap();
        let instance_dir = temp.path();

        let args = vec!["--username".to_string(), "Player".to_string()];
        let argfile = create_argfile(&args, instance_dir).expect("create argfile failed");
        assert!(argfile.exists());
        assert!(argfile
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("args-"));

        // Simulate startup sweep
        for entry in std::fs::read_dir(instance_dir).unwrap().flatten() {
            let p = entry.path();
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                if name.starts_with("args-") && name.ends_with(".txt") {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
        assert!(!argfile.exists(), "Argfile should be swept on startup");
    }

    #[test]
    fn test_n02_single_quote_custom_jvm_arg_no_panic() {
        let args = get_preset_jvm_args(JvmPreset::Custom, "\"");
        assert!(args.is_empty());
        let args2 = get_preset_jvm_args(JvmPreset::Custom, "-DFoo=bar '");
        assert_eq!(args2, vec!["-DFoo=bar".to_string()]);
    }
}
