use crate::{get_system_memory, DataPaths};
use chrono::Utc;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use launcher_types::LauncherErrorPayload;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

const MAX_MEMORY_LINES: usize = 1500;
const MAX_DISK_BYTES: u64 = 2 * 1024 * 1024;
const DIAGNOSTIC_PREFIX: &str = "MCSE1:";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LauncherLogLine {
    pub stream: String,
    pub line: String,
    pub time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReportResult {
    pub ref_id: String,
    pub code: String,
    pub file_path: String,
    pub report_text: String,
    pub launcher_lines: usize,
    pub game_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticInstanceMeta {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactDiagnosticPayload {
    v: u8,
    ref_id: String,
    created_at: String,
    app_version: String,
    os: String,
    arch: String,
    memory_total_mb: u64,
    memory_free_mb: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance: Option<DiagnosticInstanceMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    launcher_tail: Vec<String>,
    game_tail: Vec<String>,
}

struct InnerState {
    buffer: VecDeque<LauncherLogLine>,
    log_file_path: PathBuf,
    logs_dir: PathBuf,
}

#[derive(Clone)]
pub struct LauncherLogStore {
    inner: Arc<Mutex<InnerState>>,
}

impl LauncherLogStore {
    pub fn new(paths: &DataPaths) -> Self {
        let logs_dir = paths.logs_dir.clone();
        let _ = fs::create_dir_all(&logs_dir);
        let log_file_path = logs_dir.join("launcher.log");

        rotate_if_oversized(&log_file_path, &logs_dir.join("launcher.prev.log"));

        let mut buffer = VecDeque::with_capacity(MAX_MEMORY_LINES);
        for line in read_tail_lines(&log_file_path, 150) {
            if let Some(parsed) = parse_persisted_line(&line) {
                buffer.push_back(parsed);
            }
        }

        let store = Self {
            inner: Arc::new(Mutex::new(InnerState {
                buffer,
                log_file_path,
                logs_dir,
            })),
        };

        store.record_entry(
            "system",
            format!(
                "=== Launcher session started (v{}, {} {}) ===",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        );

        store
    }

    pub fn record_entry(&self, stream: &str, message: impl AsRef<str>) -> Option<LauncherLogLine> {
        let sanitized = sanitize_log_text(message.as_ref().trim_end_matches(['\r', '\n']));
        if sanitized.trim().is_empty() {
            return None;
        }

        let stream_norm = match stream {
            "stderr" | "error" => "stderr",
            "system" | "warn" => "system",
            _ => "stdout",
        };

        let entry = LauncherLogLine {
            stream: stream_norm.to_string(),
            line: sanitized,
            time: Utc::now().to_rfc3339(),
        };

        if let Ok(mut guard) = self.inner.lock() {
            if guard.buffer.len() >= MAX_MEMORY_LINES {
                guard.buffer.pop_front();
            }
            guard.buffer.push_back(entry.clone());
            append_entry_to_disk(&guard.log_file_path, &entry);
        }

        Some(entry)
    }

    pub fn history(&self) -> Vec<LauncherLogLine> {
        self.inner
            .lock()
            .map(|guard| guard.buffer.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear_memory(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.buffer.clear();
        }
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.inner
            .lock()
            .map(|guard| guard.logs_dir.clone())
            .unwrap_or_else(|_| crate::resolve_data_root().join("logs"))
    }

    pub fn create_diagnostic_report(
        &self,
        instance: Option<DiagnosticInstanceMeta>,
        game_lines: Vec<LauncherLogLine>,
        fallback_instance_log_path: Option<PathBuf>,
        last_error: Option<String>,
    ) -> Result<DiagnosticReportResult, LauncherErrorPayload> {
        let now = Utc::now();
        let mem = get_system_memory();

        let (logs_dir, log_file_path, memory_lines) = {
            let guard = self.inner.lock().map_err(|_| {
                LauncherErrorPayload::internal("Gagal mengakses buffer log launcher.")
            })?;
            (
                guard.logs_dir.clone(),
                guard.log_file_path.clone(),
                guard.buffer.iter().cloned().collect::<Vec<_>>(),
            )
        };

        let launcher_formatted: Vec<String> = if !memory_lines.is_empty() {
            memory_lines
                .iter()
                .rev()
                .take(220)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(format_log_line)
                .collect()
        } else {
            read_tail_lines(&log_file_path, 220)
                .into_iter()
                .map(|l| sanitize_log_text(&l))
                .collect()
        };

        let game_formatted: Vec<String> = if !game_lines.is_empty() {
            game_lines
                .iter()
                .rev()
                .take(260)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|entry| {
                    let clean = LauncherLogLine {
                        stream: entry.stream.clone(),
                        line: sanitize_log_text(&entry.line),
                        time: entry.time.clone(),
                    };
                    format_log_line(&clean)
                })
                .collect()
        } else if let Some(ref mc_latest) = fallback_instance_log_path {
            read_tail_lines(mc_latest, 260)
                .into_iter()
                .map(|l| sanitize_log_text(&l))
                .collect()
        } else {
            Vec::new()
        };

        let crash_tail = read_optional_error_file(&logs_dir.join("crash.log"), 40);
        let launch_err_tail = read_optional_error_file(&logs_dir.join("launch_error.log"), 40);

        let clean_last_error = last_error
            .map(|e| sanitize_log_text(e.trim()))
            .filter(|e| !e.is_empty());

        let clean_instance = instance.map(|inst| DiagnosticInstanceMeta {
            id: sanitize_log_text(&inst.id),
            name: sanitize_log_text(&inst.name),
            minecraft_version: sanitize_log_text(&inst.minecraft_version),
            loader: sanitize_log_text(&inst.loader),
        });

        let digest_input = format!(
            "{}|{}|{}|{}",
            now.to_rfc3339(),
            clean_last_error.as_deref().unwrap_or(""),
            launcher_formatted.last().map(String::as_str).unwrap_or(""),
            game_formatted.last().map(String::as_str).unwrap_or("")
        );
        let hash_hex = hex::encode(Sha256::digest(digest_input.as_bytes()));
        let short_hash = &hash_hex[..6].to_ascii_uppercase();
        let ref_id = format!("MCL-{}-{}", now.format("%Y%m%d-%H%M%S"), short_hash);

        let compact_payload = CompactDiagnosticPayload {
            v: 1,
            ref_id: ref_id.clone(),
            created_at: now.to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            memory_total_mb: mem.total_mb,
            memory_free_mb: mem.free_mb,
            instance: clean_instance.clone(),
            last_error: clean_last_error.clone(),
            launcher_tail: launcher_formatted
                .iter()
                .rev()
                .take(45)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
            game_tail: game_formatted
                .iter()
                .rev()
                .take(60)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
        };

        let code = encode_diagnostic_code(&compact_payload)?;

        let mut report = String::new();
        report.push_str("=== MSE LAUNCHER - OFFLINE DIAGNOSTIC REPORT ===\n");
        report.push_str(&format!("Ref ID       : {}\n", ref_id));
        report.push_str(&format!("Created At   : {}\n", now.to_rfc3339()));
        report.push_str(&format!("App Version  : {}\n", env!("CARGO_PKG_VERSION")));
        report.push_str(&format!(
            "Platform     : {} ({})\n",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        report.push_str(&format!(
            "Memory       : {} MB free / {} MB total\n",
            mem.free_mb, mem.total_mb
        ));
        if let Some(ref inst) = clean_instance {
            report.push_str(&format!(
                "Instance     : {} (id={}, mc={}, loader={})\n",
                inst.name, inst.id, inst.minecraft_version, inst.loader
            ));
        }
        if let Some(ref err) = clean_last_error {
            report.push_str(&format!("Last Error   : {}\n", err));
        }
        report.push('\n');
        report.push_str("=== OFFLINE DIAGNOSTIC CODE ===\n");
        report.push_str(&code);
        report.push_str("\n\n");

        if !crash_tail.is_empty() {
            report.push_str("=== PANIC CRASH LOG ===\n");
            for line in &crash_tail {
                report.push_str(line);
                report.push('\n');
            }
            report.push('\n');
        }

        if !launch_err_tail.is_empty() {
            report.push_str("=== STARTUP ERROR LOG ===\n");
            for line in &launch_err_tail {
                report.push_str(line);
                report.push('\n');
            }
            report.push('\n');
        }

        report.push_str(&format!(
            "=== LAUNCHER LOGS ({} lines) ===\n",
            launcher_formatted.len()
        ));
        if launcher_formatted.is_empty() {
            report.push_str("(no launcher logs captured)\n");
        } else {
            for line in &launcher_formatted {
                report.push_str(line);
                report.push('\n');
            }
        }
        report.push('\n');

        report.push_str(&format!(
            "=== MINECRAFT GAME LOGS ({} lines) ===\n",
            game_formatted.len()
        ));
        if game_formatted.is_empty() {
            report.push_str("(no game logs captured)\n");
        } else {
            for line in &game_formatted {
                report.push_str(line);
                report.push('\n');
            }
        }

        fs::create_dir_all(&logs_dir).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal membuat folder log: {}", e))
        })?;
        let report_file = logs_dir.join(format!("diagnostic-{}.txt", ref_id));
        fs::write(&report_file, &report).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal menulis file laporan log: {}", e))
        })?;

        Ok(DiagnosticReportResult {
            ref_id,
            code,
            file_path: sanitize_log_text(&report_file.to_string_lossy()),
            report_text: report,
            launcher_lines: launcher_formatted.len(),
            game_lines: game_formatted.len(),
        })
    }
}

fn format_log_line(entry: &LauncherLogLine) -> String {
    let time_short = if entry.time.len() >= 19 {
        &entry.time[..19]
    } else {
        entry.time.as_str()
    };
    format!("[{}] [{}] {}", time_short, entry.stream, entry.line)
}

fn append_entry_to_disk(path: &PathBuf, entry: &LauncherLogLine) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let backup = path.with_file_name("launcher.prev.log");
    rotate_if_oversized(path, &backup);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{}] [{}] {}", entry.time, entry.stream, entry.line);
    }
}

fn rotate_if_oversized(current: &PathBuf, backup: &PathBuf) {
    if let Ok(meta) = fs::metadata(current) {
        if meta.len() > MAX_DISK_BYTES {
            let _ = fs::remove_file(backup);
            let _ = fs::rename(current, backup);
        }
    }
}

fn parse_persisted_line(raw: &str) -> Option<LauncherLogLine> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((time, after_time)) = rest.split_once("] [") {
            if let Some((stream, line)) = after_time.split_once("] ") {
                let norm_stream = match stream {
                    "stderr" | "system" | "stdout" => stream,
                    _ => "stdout",
                };
                return Some(LauncherLogLine {
                    time: time.to_string(),
                    stream: norm_stream.to_string(),
                    line: sanitize_log_text(line),
                });
            }
        }
    }
    Some(LauncherLogLine {
        time: Utc::now().to_rfc3339(),
        stream: "stdout".to_string(),
        line: sanitize_log_text(trimmed),
    })
}

fn read_tail_lines(path: &PathBuf, max_lines: usize) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut ring: VecDeque<String> = VecDeque::with_capacity(max_lines);
    for line in content.lines() {
        if ring.len() >= max_lines {
            ring.pop_front();
        }
        ring.push_back(line.to_string());
    }
    ring.into_iter().collect()
}

fn read_optional_error_file(path: &PathBuf, max_lines: usize) -> Vec<String> {
    read_tail_lines(path, max_lines)
        .into_iter()
        .map(|l| sanitize_log_text(&l))
        .collect()
}

pub fn strip_ansi(input: &str) -> String {
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    let re =
        ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").expect("valid ansi regex"));
    re.replace_all(input, "").into_owned()
}

/// Menyensor token, API key, kredensial, serta nama folder user Windows/Unix sebelum disimpan/dibagikan.
pub fn sanitize_log_text(input: &str) -> String {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            (
                Regex::new(r#"(?i)(access[_-]?token|refresh[_-]?token|x-api-key|client[_-]?secret|mc_access_token)\s*([:=])\s*([^\s,"']+)"#)
                    .expect("valid token regex"),
                "$1$2[REDACTED]",
            ),
            (
                Regex::new(r#"(?i)("(?:accessToken|refreshToken|x-api-key|clientSecret|apiKey)"\s*:\s*")([^"]+)(")"#)
                    .expect("valid json secret regex"),
                "$1[REDACTED]$3",
            ),
            (
                Regex::new(r#"(?i)(--accessToken\s+)(\S+)"#).expect("valid cli token regex"),
                "$1[REDACTED]",
            ),
            (
                Regex::new(r#"(?i)(Bearer\s+)[A-Za-z0-9\-._~+/]+=*"#).expect("valid bearer regex"),
                "$1[REDACTED]",
            ),
            (
                Regex::new(r"(?i)([A-Z]:\\Users\\)([^\\/:*?\x22<>|\r\n]+)(\\)")
                    .expect("valid win path with backslash regex"),
                "$1<USER>$3",
            ),
            (
                Regex::new(r"(?i)([A-Z]:/Users/)([^/:*?\x22<>|\r\n]+)(/)")
                    .expect("valid win slash path with trailing slash regex"),
                "$1<USER>$3",
            ),
            (
                Regex::new(r"(/(?:Users|home)/)([^/\r\n]+)(/)")
                    .expect("valid unix path with slash regex"),
                "$1<USER>$3",
            ),
            (
                Regex::new(r"(?i)([A-Z]:\\Users\\)([^\\/:*?\x22<>|\s]+)").expect("valid win path regex"),
                "$1<USER>",
            ),
            (
                Regex::new(r"(?i)([A-Z]:/Users/)([^/:*?\x22<>|\s]+)").expect("valid win slash path regex"),
                "$1<USER>",
            ),
            (
                Regex::new(r"(/(?:Users|home)/)([^/\s]+)").expect("valid unix path regex"),
                "$1<USER>",
            ),
        ]
    });

    let mut out = strip_ansi(input);
    if let Ok(override_root) = std::env::var("MCLAUNCHER_DATA_ROOT") {
        let trimmed = override_root.trim();
        if !trimmed.is_empty() {
            out = out.replace(trimmed, "<DATA_ROOT>");
        }
    }
    for (re, replacement) in patterns {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

fn encode_diagnostic_code(
    payload: &CompactDiagnosticPayload,
) -> Result<String, LauncherErrorPayload> {
    let json = serde_json::to_vec(payload)
        .map_err(|e| LauncherErrorPayload::internal(format!("Gagal serialisasi laporan: {}", e)))?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&json)
        .and_then(|_| encoder.finish())
        .map_err(|e| LauncherErrorPayload::internal(format!("Gagal kompresi laporan: {}", e)))
        .map(|compressed| format!("{}{}", DIAGNOSTIC_PREFIX, encode_urlsafe_b64(&compressed)))
}

pub fn decode_diagnostic_code(code: &str) -> Result<String, LauncherErrorPayload> {
    let raw = code.trim().strip_prefix(DIAGNOSTIC_PREFIX).ok_or_else(|| {
        LauncherErrorPayload::invalid_input("Format kode diagnostik tidak valid.")
    })?;
    let bytes = decode_urlsafe_b64(raw).ok_or_else(|| {
        LauncherErrorPayload::invalid_input("Encoding Base64 kode diagnostik rusak.")
    })?;
    let mut decoder = DeflateDecoder::new(&bytes[..]);
    let mut out = String::new();
    decoder.read_to_string(&mut out).map_err(|e| {
        LauncherErrorPayload::invalid_input(format!("Gagal mendekompresi kode: {}", e))
    })?;
    Ok(out)
}

const B64_URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn encode_urlsafe_b64(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() * 4).div_ceil(3));
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_URL_ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_URL_ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_URL_ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(B64_URL_ALPHABET[(triple & 0x3F) as usize] as char);
        }
    }
    out
}

fn decode_urlsafe_b64(input: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = input
        .as_bytes()
        .iter()
        .copied()
        .filter(|b| *b != b'=')
        .collect();
    if bytes.is_empty() || bytes.len() % 4 == 1 {
        return None;
    }

    let val = |b: u8| -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a' + 26) as u32),
            b'0'..=b'9' => Some((b - b'0' + 52) as u32),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    };

    let mut out = Vec::with_capacity((bytes.len() * 3) / 4);
    for chunk in bytes.chunks(4) {
        let c0 = val(chunk[0])?;
        let c1 = val(*chunk.get(1)?)?;
        let c2 = match chunk.get(2) {
            Some(&b) => Some(val(b)?),
            None => None,
        };
        let c3 = match chunk.get(3) {
            Some(&b) => Some(val(b)?),
            None => None,
        };

        let triple = (c0 << 18) | (c1 << 12) | (c2.unwrap_or(0) << 6) | c3.unwrap_or(0);
        out.push(((triple >> 16) & 0xFF) as u8);
        if c2.is_some() {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if c3.is_some() {
            out.push((triple & 0xFF) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_sanitize_redacts_tokens_and_usernames() {
        let raw = r#"Auth failed for C:\Users\ExampleUser\AppData\Roaming\MCLauncher with --accessToken eyJhbGciOi.123.456 Bearer abcdef12345 {"refreshToken":"secret_ref"}"#;
        let clean = sanitize_log_text(raw);
        assert!(!clean.contains("ExampleUser"));
        assert!(clean.contains(r"C:\Users\<USER>\AppData"));
        assert!(!clean.contains("eyJhbGciOi"));
        assert!(!clean.contains("abcdef12345"));
        assert!(!clean.contains("secret_ref"));
        assert!(clean.contains("[REDACTED]"));
    }

    #[test]
    fn test_diagnostic_report_writes_file_and_roundtrips_code() {
        let root = std::env::temp_dir().join(format!("mcl-logs-test-{}", Uuid::new_v4()));
        let paths = DataPaths::new(&root);
        let store = LauncherLogStore::new(&paths);
        store.record_entry("stdout", "Preparing Java runtime");
        store.record_entry(
            "stderr",
            r"Error opening C:\Users\Tester\secret --accessToken tok_xyz",
        );

        let game_lines = vec![
            LauncherLogLine {
                stream: "stdout".to_string(),
                line: "[main/INFO]: Loading Minecraft 1.20.4".to_string(),
                time: "2026-11-13T10:00:00Z".to_string(),
            },
            LauncherLogLine {
                stream: "stderr".to_string(),
                line: "java.lang.OutOfMemoryError".to_string(),
                time: "2026-11-13T10:00:05Z".to_string(),
            },
        ];

        let result = store
            .create_diagnostic_report(
                Some(DiagnosticInstanceMeta {
                    id: "survival-1".to_string(),
                    name: "Survival".to_string(),
                    minecraft_version: "1.20.4".to_string(),
                    loader: "fabric".to_string(),
                }),
                game_lines,
                None,
                Some("Game exited with code 1".to_string()),
            )
            .expect("create_diagnostic_report must succeed");

        assert!(result.ref_id.starts_with("MCL-"));
        assert!(result.code.starts_with(DIAGNOSTIC_PREFIX));
        assert!(paths
            .logs_dir
            .join(format!("diagnostic-{}.txt", result.ref_id))
            .exists());
        assert!(!result.report_text.contains("tok_xyz"));
        assert!(!result.report_text.contains(r"\Tester\"));

        let decoded_json = decode_diagnostic_code(&result.code).expect("code must decode offline");
        let parsed: serde_json::Value = serde_json::from_str(&decoded_json).unwrap();
        assert_eq!(parsed["refId"], result.ref_id);
        assert_eq!(parsed["lastError"], "Game exited with code 1");
        assert!(!decoded_json.contains("tok_xyz"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_launcher_log_rotates_oversized_file() {
        let root = std::env::temp_dir().join(format!("mcl-logs-rot-{}", Uuid::new_v4()));
        let paths = DataPaths::new(&root);
        fs::create_dir_all(&paths.logs_dir).unwrap();
        let big = vec![b'a'; (MAX_DISK_BYTES as usize) + 128];
        fs::write(paths.logs_dir.join("launcher.log"), &big).unwrap();

        let store = LauncherLogStore::new(&paths);
        assert!(paths.logs_dir.join("launcher.prev.log").exists());
        let hist = store.history();
        assert!(!hist.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_ps14_sanitize_log_text_handles_spaces_in_username() {
        let win_msg =
            r#"Failed at C:\Users\Alice Smith\AppData\Roaming\MCLauncher\logs\latest.log"#;
        let unix_msg = "Failed at /home/alice smith/MCLauncher/logs/latest.log";
        let clean_win = sanitize_log_text(win_msg);
        let clean_unix = sanitize_log_text(unix_msg);
        assert!(!clean_win.contains("Alice Smith"));
        assert!(clean_win.contains(r"C:\Users\<USER>\AppData"));
        assert!(!clean_unix.contains("alice smith"));
        assert!(clean_unix.contains("/home/<USER>/MCLauncher"));
    }
}
