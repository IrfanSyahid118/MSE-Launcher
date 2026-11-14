use chrono::Utc;
use launcher_types::LauncherErrorPayload;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub stream: String, // "stdout" | "stderr" | "system"
    pub line: String,
    pub time: String,
}

pub struct RunningGame {
    pub pid: u32,
    kill_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl RunningGame {
    pub async fn kill(&self) -> Result<(), LauncherErrorPayload> {
        let mut tx_opt = self.kill_tx.lock().await;
        if let Some(tx) = tx_opt.take() {
            let _ = tx.send(());
        }
        Ok(())
    }
}

pub async fn spawn_game_process<F1, F2>(
    executable: &Path,
    args: &[String],
    working_dir: &Path,
    on_log: F1,
    on_exit: F2,
) -> Result<RunningGame, LauncherErrorPayload>
where
    F1: Fn(Vec<LogLine>) + Send + Sync + 'static,
    F2: Fn(Option<i32>) + Send + Sync + 'static,
{
    let mut cmd = Command::new(executable);
    cmd.args(args);
    cmd.current_dir(working_dir);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // SEC-04 & SEC-07: Bersihkan variabel environment yang dapat memanipulasi JVM atau menginjeksi opsi/agen liar
    cmd.env_remove("JAVA_TOOL_OPTIONS");
    cmd.env_remove("_JAVA_OPTIONS");
    cmd.env_remove("JDK_JAVA_OPTIONS");
    cmd.env_remove("CLASSPATH");

    // FN-01: JANGAN gunakan CREATE_NO_WINDOW pada javaw.exe karena menyebabkan
    // jendela game GLFW/LWJGL tidak dapat dirender atau disembunyikan oleh Windows.

    let mut child = cmd.spawn().map_err(|e| {
        LauncherErrorPayload::internal(format!(
            "Gagal meluncurkan game dengan executable {:?}: {}",
            executable, e
        ))
    })?;

    let pid = child.id().unwrap_or(0);

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
    let kill_tx = Arc::new(Mutex::new(Some(kill_tx)));
    let log_cb = Arc::new(on_log);

    // FN-05: 100ms batch buffer untuk mencegah banjir event IPC saat startup game
    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<LogLine>(2048);

    {
        let cb = Arc::clone(&log_cb);
        tokio::spawn(async move {
            let mut buffer = Vec::new();
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    res = log_rx.recv() => {
                        match res {
                            Some(log_line) => {
                                buffer.push(log_line);
                                if buffer.len() >= 50 {
                                    cb(std::mem::take(&mut buffer));
                                }
                            }
                            None => {
                                if !buffer.is_empty() {
                                    cb(std::mem::take(&mut buffer));
                                }
                                break;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            cb(std::mem::take(&mut buffer));
                        }
                    }
                }
            }
        });
    }

    // Stream stdout
    if let Some(out) = stdout {
        let tx = log_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx
                    .send(LogLine {
                        stream: "stdout".to_string(),
                        line,
                        time: Utc::now().to_rfc3339(),
                    })
                    .await;
            }
        });
    }

    // Stream stderr
    if let Some(err) = stderr {
        let tx = log_tx;
        tokio::spawn(async move {
            let mut reader = BufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx
                    .send(LogLine {
                        stream: "stderr".to_string(),
                        line,
                        time: Utc::now().to_rfc3339(),
                    })
                    .await;
            }
        });
    }

    // Monitor exit and handle kill signal without holding locks across wait()
    tokio::spawn(async move {
        tokio::select! {
            status = child.wait() => {
                let code = status.ok().and_then(|s| s.code());
                on_exit(code);
            }
            _ = kill_rx => {
                let _ = child.start_kill();
                let status = child.wait().await;
                let code = status.ok().and_then(|s| s.code());
                on_exit(code);
            }
        }
    });

    Ok(RunningGame { pid, kill_tx })
}
