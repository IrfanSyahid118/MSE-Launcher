use launcher_platform::DataPaths;
pub use launcher_platform::{
    strip_ansi, DiagnosticInstanceMeta, DiagnosticReportResult, LauncherLogLine, LauncherLogStore,
};
use launcher_types::LauncherErrorPayload;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tracing_subscriber::fmt::MakeWriter;

struct EmitterState {
    pending_emit: Vec<LauncherLogLine>,
    app_handle: Option<AppHandle>,
}

#[derive(Clone)]
pub struct LauncherLogService {
    store: LauncherLogStore,
    emitter: Arc<Mutex<EmitterState>>,
}

impl LauncherLogService {
    pub fn new(paths: &DataPaths) -> Self {
        Self {
            store: LauncherLogStore::new(paths),
            emitter: Arc::new(Mutex::new(EmitterState {
                pending_emit: Vec::new(),
                app_handle: None,
            })),
        }
    }

    /// Menghubungkan `AppHandle` dan menjalankan pengirim batch event `launcher:log`.
    pub fn attach_app_handle(&self, app: AppHandle) {
        if let Ok(mut guard) = self.emitter.lock() {
            guard.app_handle = Some(app);
            guard.pending_emit.clear();
        }

        let weak_emitter = Arc::downgrade(&self.emitter);
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(120));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                let Some(emitter) = weak_emitter.upgrade() else {
                    break;
                };

                let (app_opt, batch) = {
                    let Ok(mut guard) = emitter.lock() else {
                        continue;
                    };
                    if guard.pending_emit.is_empty() {
                        continue;
                    }
                    let app_opt = guard.app_handle.clone();
                    let batch = std::mem::take(&mut guard.pending_emit);
                    (app_opt, batch)
                };

                if let Some(app) = app_opt {
                    let _ = app.emit("launcher:log", batch);
                }
            }
        });
    }

    pub fn make_tracing_writer(&self) -> LauncherMakeWriter {
        LauncherMakeWriter {
            service: self.clone(),
        }
    }

    pub fn record_entry(&self, stream: &str, message: impl AsRef<str>) {
        if let Some(entry) = self.store.record_entry(stream, message) {
            if let Ok(mut guard) = self.emitter.lock() {
                if guard.app_handle.is_some() {
                    if guard.pending_emit.len() >= 200 {
                        guard.pending_emit.drain(..100);
                    }
                    guard.pending_emit.push(entry);
                }
            }
        }
    }

    pub fn ingest_tracing_line(&self, raw_line: &str) {
        let stripped = strip_ansi(raw_line).trim().to_string();
        if stripped.is_empty() {
            return;
        }
        let stream = detect_tracing_stream(&stripped);
        self.record_entry(stream, stripped);
    }

    pub fn history(&self) -> Vec<LauncherLogLine> {
        self.store.history()
    }

    pub fn clear_memory(&self) {
        self.store.clear_memory();
        if let Ok(mut guard) = self.emitter.lock() {
            guard.pending_emit.clear();
        }
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.store.logs_dir()
    }

    pub fn create_diagnostic_report(
        &self,
        instance: Option<DiagnosticInstanceMeta>,
        game_lines: Vec<LauncherLogLine>,
        fallback_instance_log_path: Option<PathBuf>,
        last_error: Option<String>,
    ) -> Result<DiagnosticReportResult, LauncherErrorPayload> {
        let result = self.store.create_diagnostic_report(
            instance,
            game_lines,
            fallback_instance_log_path,
            last_error,
        )?;
        self.record_entry(
            "system",
            format!(
                "Laporan diagnostik offline dibuat: {} (diagnostic-{}.txt)",
                result.ref_id, result.ref_id
            ),
        );
        Ok(result)
    }
}

pub struct LauncherMakeWriter {
    service: LauncherLogService,
}

impl<'a> MakeWriter<'a> for LauncherMakeWriter {
    type Writer = TeeLineWriter;

    fn make_writer(&'a self) -> Self::Writer {
        TeeLineWriter {
            service: self.service.clone(),
            line_buf: Vec::with_capacity(256),
        }
    }
}

pub struct TeeLineWriter {
    service: LauncherLogService,
    line_buf: Vec<u8>,
}

impl TeeLineWriter {
    fn flush_line_buf(&mut self) {
        if self.line_buf.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(&self.line_buf);
        for part in text.lines() {
            self.service.ingest_tracing_line(part);
        }
        self.line_buf.clear();
    }
}

impl Write for TeeLineWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        self.line_buf.extend_from_slice(buf);
        if buf.contains(&b'\n') {
            self.flush_line_buf();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        self.flush_line_buf();
        Ok(())
    }
}

impl Drop for TeeLineWriter {
    fn drop(&mut self) {
        self.flush_line_buf();
    }
}

fn detect_tracing_stream(line: &str) -> &'static str {
    if line.contains(" ERROR ") || line.starts_with("ERROR ") {
        "stderr"
    } else if line.contains(" WARN ") || line.starts_with("WARN ") {
        "system"
    } else {
        "stdout"
    }
}
