use chrono::Utc;
use launcher_types::LauncherErrorPayload;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub stream: String,
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

    cmd.env_remove("JAVA_TOOL_OPTIONS");
    cmd.env_remove("_JAVA_OPTIONS");
    cmd.env_remove("JDK_JAVA_OPTIONS");
    cmd.env_remove("CLASSPATH");

    // Do not set CREATE_NO_WINDOW on javaw.exe; it hides LWJGL/GLFW game windows on Windows.

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

    // Batch logs every 100ms to avoid IPC event flooding during game startup.
    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<LogLine>(2048);

    let batcher_handle = {
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
        })
    };

    let stdout_handle = stdout.map(|out| {
        let tx = log_tx.clone();
        tokio::spawn(pump_log_stream("stdout", out, tx))
    });

    let stderr_handle = stderr.map(|err| {
        let tx = log_tx.clone();
        tokio::spawn(pump_log_stream("stderr", err, tx))
    });

    // Drop the original sender so `log_rx` closes once all stream readers finish.
    drop(log_tx);

    tokio::spawn(async move {
        let code = tokio::select! {
            status = child.wait() => status.ok().and_then(|s| s.code()),
            _ = kill_rx => {
                let _ = child.start_kill();
                let status = child.wait().await;
                status.ok().and_then(|s| s.code())
            }
        };

        let mut stdout_handle = stdout_handle;
        let mut stderr_handle = stderr_handle;
        let mut batcher_handle = batcher_handle;

        let drained = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            if let Some(handle) = stdout_handle.as_mut() {
                let _ = handle.await;
            }
            if let Some(handle) = stderr_handle.as_mut() {
                let _ = handle.await;
            }
            let _ = (&mut batcher_handle).await;
        })
        .await;

        // Proses anak yang mewarisi pipe bisa menahan EOF; hentikan pembaca agar tidak bocor per launch.
        if drained.is_err() {
            if let Some(handle) = stdout_handle.as_ref() {
                handle.abort();
            }
            if let Some(handle) = stderr_handle.as_ref() {
                handle.abort();
            }
            batcher_handle.abort();
        }

        on_exit(code);
    });

    Ok(RunningGame { pid, kill_tx })
}

async fn pump_log_stream<R: AsyncRead + Unpin>(
    stream_name: &'static str,
    reader: R,
    tx: tokio::sync::mpsc::Sender<LogLine>,
) {
    let mut buf_reader = BufReader::new(reader);
    let mut buf = Vec::with_capacity(512);

    loop {
        buf.clear();
        match buf_reader.read_until(b'\n', &mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                while matches!(buf.last(), Some(b'\n' | b'\r')) {
                    buf.pop();
                }
                let line = String::from_utf8_lossy(&buf).into_owned();
                if tx
                    .send(LogLine {
                        stream: stream_name.to_string(),
                        line,
                        time: Utc::now().to_rfc3339(),
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
