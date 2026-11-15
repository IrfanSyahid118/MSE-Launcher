use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use launcher_net::HttpClient;
use launcher_types::LauncherErrorPayload;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub url: String,
    pub destination: PathBuf,
    pub expected_sha1: Option<String>,
    pub expected_sha256: Option<String>,
    pub expected_size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub completed_files: usize,
    pub total_files: usize,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub current_file: String,
}

struct TempFileGuard {
    path: PathBuf,
    disarmed: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            disarmed: false,
        }
    }

    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub async fn verify_local_file(
    path: &Path,
    expected_size: Option<u64>,
    expected_sha1: Option<&str>,
) -> bool {
    verify_local_file_hashes(path, expected_size, expected_sha1, None).await
}

pub async fn verify_local_file_hashes(
    path: &Path,
    expected_size: Option<u64>,
    expected_sha1: Option<&str>,
    expected_sha256: Option<&str>,
) -> bool {
    if !path.exists() {
        return false;
    }

    if let Some(size) = expected_size {
        if let Ok(meta) = fs::metadata(path).await {
            if meta.len() != size {
                return false;
            }
        } else {
            return false;
        }
    }

    if expected_sha1.is_some() || expected_sha256.is_some() {
        use tokio::io::AsyncReadExt;
        let Ok(mut file) = fs::File::open(path).await else {
            return false;
        };
        let mut sha1_hasher = expected_sha1.map(|_| Sha1::new());
        let mut sha256_hasher = expected_sha256.map(|_| Sha256::new());
        let mut buf = [0u8; 64 * 1024];

        loop {
            match file.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Some(chunk) = buf.get(..n) {
                        if let Some(ref mut h) = sha1_hasher {
                            h.update(chunk);
                        }
                        if let Some(ref mut h) = sha256_hasher {
                            h.update(chunk);
                        }
                    }
                }
                Err(_) => return false,
            }
        }

        if let (Some(expected), Some(hasher)) = (expected_sha1, sha1_hasher) {
            let hex_hash = hex::encode(hasher.finalize());
            if !hex_hash.eq_ignore_ascii_case(expected) {
                return false;
            }
        }

        if let (Some(expected), Some(hasher)) = (expected_sha256, sha256_hasher) {
            let hex_hash = hex::encode(hasher.finalize());
            if !hex_hash.eq_ignore_ascii_case(expected) {
                return false;
            }
        }
    }

    true
}

async fn replace_destination(
    temp_path: &Path,
    destination: &Path,
) -> Result<(), LauncherErrorPayload> {
    if let Err(e) = fs::rename(temp_path, destination).await {
        return Err(LauncherErrorPayload::file_system(format!(
            "Gagal mengganti nama {:?} ke {:?}: {}",
            temp_path, destination, e
        )));
    }

    Ok(())
}

async fn wait_cancelled(cancelled: Option<&Arc<AtomicBool>>) {
    let Some(flag) = cancelled else {
        std::future::pending::<()>().await;
        return;
    };
    while !flag.load(Ordering::Relaxed) {
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
}

fn rollback_bytes(counter: Option<&Arc<AtomicU64>>, delta: u64) {
    if delta == 0 {
        return;
    }
    if let Some(c) = counter {
        let _ = c.try_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
            Some(cur.saturating_sub(delta))
        });
    }
}

fn bounded_temp_filename(dest: &Path) -> String {
    let stem = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let suffix = format!(".tmp-{}", uuid::Uuid::new_v4().simple());
    let mut end = stem.len().min(200);
    // Walk back to a UTF-8 char boundary so the temp filename always stays <= 255 bytes.
    while end > 0 && !stem.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &stem[..end], suffix)
}

fn normalized_dest_key(path: &Path) -> String {
    let mut cleaned = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            other => cleaned.push(other.as_os_str()),
        }
    }
    let s = cleaned.to_string_lossy();
    #[cfg(windows)]
    {
        s.replace('\\', "/").to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        s.into_owned()
    }
}

fn hashes_conflict(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => !x.eq_ignore_ascii_case(y),
        _ => false,
    }
}

fn deduplicate_tasks(tasks: Vec<DownloadTask>) -> Result<Vec<DownloadTask>, LauncherErrorPayload> {
    let mut deduped: Vec<DownloadTask> = Vec::with_capacity(tasks.len());
    let mut index_by_dest: std::collections::HashMap<String, usize> =
        std::collections::HashMap::with_capacity(tasks.len());

    for task in tasks {
        let key = normalized_dest_key(&task.destination);
        if let Some(&idx) = index_by_dest.get(&key) {
            let existing = &mut deduped[idx];
            if hashes_conflict(&existing.expected_sha1, &task.expected_sha1)
                || hashes_conflict(&existing.expected_sha256, &task.expected_sha256)
                || existing
                    .expected_size
                    .zip(task.expected_size)
                    .is_some_and(|(a, b)| a != b)
            {
                return Err(LauncherErrorPayload::invalid_input(format!(
                    "Konflik target unduhan untuk tujuan yang sama: {:?}",
                    task.destination
                )));
            }
            if existing.expected_sha1.is_none() {
                existing.expected_sha1 = task.expected_sha1;
            }
            if existing.expected_sha256.is_none() {
                existing.expected_sha256 = task.expected_sha256;
            }
            if existing.expected_size.is_none() {
                existing.expected_size = task.expected_size;
            }
            continue;
        }
        index_by_dest.insert(key, deduped.len());
        deduped.push(task);
    }

    Ok(deduped)
}

#[derive(Clone)]
pub struct DownloadManager {
    http: HttpClient,
    parallelism: Arc<AtomicUsize>,
}

impl DownloadManager {
    pub fn new(parallelism: usize) -> Self {
        Self {
            http: HttpClient::new(),
            parallelism: Arc::new(AtomicUsize::new(parallelism.clamp(1, 32))),
        }
    }

    pub fn with_http_client(parallelism: usize, http: HttpClient) -> Self {
        Self {
            http,
            parallelism: Arc::new(AtomicUsize::new(parallelism.clamp(1, 32))),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_loopback_test(parallelism: usize) -> Self {
        Self {
            http: HttpClient::new_loopback_test(),
            parallelism: Arc::new(AtomicUsize::new(parallelism.clamp(1, 32))),
        }
    }

    pub fn parallelism(&self) -> usize {
        self.parallelism.load(Ordering::Relaxed)
    }

    pub fn set_parallelism(&self, parallelism: usize) {
        self.parallelism
            .store(parallelism.clamp(1, 32), Ordering::Relaxed);
    }

    pub async fn download_single(
        &self,
        task: &DownloadTask,
        bytes_counter: Option<&Arc<AtomicU64>>,
        cancelled: Option<&Arc<AtomicBool>>,
    ) -> Result<(), LauncherErrorPayload> {
        self.download_single_inner(task, bytes_counter, cancelled, None)
            .await
            .map(|_| ())
    }

    async fn download_attempt(
        &self,
        task: &DownloadTask,
        bytes_counter: Option<&Arc<AtomicU64>>,
        cancelled: Option<&Arc<AtomicBool>>,
        on_chunk: Option<&(dyn Fn() + Send + Sync)>,
    ) -> Result<(), LauncherErrorPayload> {
        let temp_path = task
            .destination
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(bounded_temp_filename(&task.destination));
        let mut temp_guard = TempFileGuard::new(temp_path.clone());

        let response = tokio::select! {
            biased;
            _ = wait_cancelled(cancelled) => {
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::Cancelled,
                    "Download dibatalkan oleh pengguna.",
                ));
            }
            res = self.http.get(&task.url) => res?,
        };
        let mut stream = response.bytes_stream();

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .await
            .map_err(|e| {
                LauncherErrorPayload::file_system(format!("Gagal membuat file sementara: {}", e))
            })?;

        let mut sha1_hasher = Sha1::new();
        let mut sha256_hasher = Sha256::new();
        let mut downloaded_bytes: u64 = 0;
        let max_allowed_bytes = task
            .expected_size
            .map(|s| s.min(MAX_DOWNLOAD_BYTES))
            .unwrap_or(MAX_DOWNLOAD_BYTES);

        let write_res: Result<(), LauncherErrorPayload> = async {
            loop {
                let maybe_chunk = tokio::select! {
                    biased;
                    _ = wait_cancelled(cancelled) => {
                        return Err(LauncherErrorPayload::new(
                            launcher_types::ErrorCode::Cancelled,
                            "Download dibatalkan oleh pengguna.",
                        ));
                    }
                    item = stream.next() => item,
                };
                let Some(chunk_result) = maybe_chunk else {
                    break;
                };

                let chunk = chunk_result.map_err(|e| {
                    LauncherErrorPayload::network(format!(
                        "Koneksi terputus saat mengunduh {}: {}",
                        task.url, e
                    ))
                })?;

                let chunk_len = chunk.len() as u64;
                downloaded_bytes = downloaded_bytes.saturating_add(chunk_len);
                if downloaded_bytes > max_allowed_bytes {
                    return Err(LauncherErrorPayload::new(
                        launcher_types::ErrorCode::HashMismatch,
                        format!(
                            "Ukuran unduhan melebihi batas yang diizinkan untuk {}",
                            task.url
                        ),
                    ));
                }

                file.write_all(&chunk).await.map_err(|e| {
                    LauncherErrorPayload::file_system(format!(
                        "Gagal menulis data ke {:?}: {}",
                        temp_path, e
                    ))
                })?;

                sha1_hasher.update(&chunk);
                sha256_hasher.update(&chunk);

                if let Some(counter) = bytes_counter {
                    counter.fetch_add(chunk_len, Ordering::Relaxed);
                }
                if let Some(notify) = on_chunk {
                    notify();
                }
            }

            file.flush().await.map_err(|e| {
                LauncherErrorPayload::file_system(format!(
                    "Gagal flush berkas {:?}: {}",
                    temp_path, e
                ))
            })?;

            Ok(())
        }
        .await;

        drop(file);
        if let Err(e) = write_res {
            rollback_bytes(bytes_counter, downloaded_bytes);
            let _ = fs::remove_file(&temp_path).await;
            return Err(e);
        }

        if let Some(expected_size) = task.expected_size {
            if downloaded_bytes != expected_size {
                rollback_bytes(bytes_counter, downloaded_bytes);
                let _ = fs::remove_file(&temp_path).await;
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::HashMismatch,
                    format!(
                        "Ukuran unduhan tidak sesuai untuk {}. Diharapkan: {} byte, Didapat: {} byte",
                        task.url, expected_size, downloaded_bytes
                    ),
                ));
            }
        }

        if let Some(ref expected_sha1) = task.expected_sha1 {
            let actual_sha1 = hex::encode(sha1_hasher.finalize());
            if !actual_sha1.eq_ignore_ascii_case(expected_sha1) {
                rollback_bytes(bytes_counter, downloaded_bytes);
                let _ = fs::remove_file(&temp_path).await;
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::HashMismatch,
                    format!(
                        "Integritas SHA-1 tidak cocok untuk {}. Diharapkan: {}, Didapat: {}",
                        task.url, expected_sha1, actual_sha1
                    ),
                ));
            }
        }

        if let Some(ref expected_sha256) = task.expected_sha256 {
            let actual_sha256 = hex::encode(sha256_hasher.finalize());
            if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
                rollback_bytes(bytes_counter, downloaded_bytes);
                let _ = fs::remove_file(&temp_path).await;
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::HashMismatch,
                    format!(
                        "Integritas SHA-256 tidak cocok untuk {}. Diharapkan: {}, Didapat: {}",
                        task.url, expected_sha256, actual_sha256
                    ),
                ));
            }
        }

        if let Err(e) = replace_destination(&temp_path, &task.destination).await {
            rollback_bytes(bytes_counter, downloaded_bytes);
            return Err(e);
        }

        temp_guard.disarm();
        Ok(())
    }

    async fn download_single_inner(
        &self,
        task: &DownloadTask,
        bytes_counter: Option<&Arc<AtomicU64>>,
        cancelled: Option<&Arc<AtomicBool>>,
        on_chunk: Option<&(dyn Fn() + Send + Sync)>,
    ) -> Result<bool, LauncherErrorPayload> {
        if let Some(c) = cancelled {
            if c.load(Ordering::Relaxed) {
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::Cancelled,
                    "Download dibatalkan oleh pengguna.",
                ));
            }
        }

        if (task.expected_sha1.is_some()
            || task.expected_sha256.is_some()
            || task.expected_size.is_some())
            && verify_local_file_hashes(
                &task.destination,
                task.expected_size,
                task.expected_sha1.as_deref(),
                task.expected_sha256.as_deref(),
            )
            .await
        {
            if let Some(size) = task.expected_size {
                if let Some(counter) = bytes_counter {
                    counter.fetch_add(size, Ordering::Relaxed);
                }
            }
            return Ok(false);
        }

        if let Some(parent) = task.destination.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                LauncherErrorPayload::file_system(format!(
                    "Gagal membuat folder {:?}: {}",
                    parent, e
                ))
            })?;
        }

        const MAX_TRANSFER_ATTEMPTS: usize = 2;
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self
                .download_attempt(task, bytes_counter, cancelled, on_chunk)
                .await
            {
                Ok(()) => return Ok(true),
                Err(e) => {
                    // `self.http.get` already handles transient request retries; only retry mid-stream disconnects or hash mismatches here.
                    let retryable = e.code == launcher_types::ErrorCode::HashMismatch
                        || (e.code == launcher_types::ErrorCode::Network
                            && e.message.starts_with("Koneksi terputus saat mengunduh "));
                    if attempt >= MAX_TRANSFER_ATTEMPTS || !retryable {
                        return Err(e);
                    }
                    tracing::warn!(
                        "Unduhan {} gagal (percobaan {}/{}): {}. Mengulang...",
                        task.url,
                        attempt,
                        MAX_TRANSFER_ATTEMPTS,
                        e.message
                    );
                    tokio::select! {
                        biased;
                        _ = wait_cancelled(cancelled) => {
                            return Err(LauncherErrorPayload::new(
                                launcher_types::ErrorCode::Cancelled,
                                "Download dibatalkan oleh pengguna.",
                            ));
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {}
                    }
                }
            }
        }
    }

    pub async fn download_all<F>(
        &self,
        tasks: Vec<DownloadTask>,
        progress_callback: F,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), LauncherErrorPayload>
    where
        F: Fn(DownloadProgress) + Send + Sync + 'static,
    {
        let tasks = deduplicate_tasks(tasks)?;
        let total_files = tasks.len();
        if total_files == 0 {
            return Ok(());
        }

        let total_bytes: u64 = tasks.iter().filter_map(|t| t.expected_size).sum();
        let completed_files = Arc::new(AtomicUsize::new(0));
        let completed_bytes = Arc::new(AtomicU64::new(0));
        let last_emit_ms = Arc::new(AtomicU64::new(0));
        let semaphore = Arc::new(Semaphore::new(self.parallelism()));
        let callback = Arc::new(progress_callback);

        let mut join_set = tokio::task::JoinSet::new();

        for task in tasks {
            let sem = Arc::clone(&semaphore);
            let comp_files = Arc::clone(&completed_files);
            let comp_bytes = Arc::clone(&completed_bytes);
            let emit_ms = Arc::clone(&last_emit_ms);
            let cb = Arc::clone(&callback);
            let cancel = Arc::clone(&cancelled);
            let dm = self.clone();

            join_set.spawn(async move {
                let _permit = sem.acquire().await.map_err(|e| {
                    LauncherErrorPayload::internal(format!(
                        "Gagal memperoleh permit semaphore: {}",
                        e
                    ))
                })?;

                let file_name = task
                    .destination
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();

                let now_ms = || -> u64 {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0)
                };

                let chunk_cb = {
                    let comp_files = Arc::clone(&comp_files);
                    let comp_bytes = Arc::clone(&comp_bytes);
                    let emit_ms = Arc::clone(&emit_ms);
                    let cb = Arc::clone(&cb);
                    let file_name = file_name.clone();
                    move || {
                        let now = now_ms();
                        let prev = emit_ms.load(Ordering::Relaxed);
                        if now.saturating_sub(prev) >= 120
                            && emit_ms
                                .compare_exchange(prev, now, Ordering::Relaxed, Ordering::Relaxed)
                                .is_ok()
                        {
                            let bytes_done = comp_bytes.load(Ordering::Relaxed);
                            cb(DownloadProgress {
                                completed_files: comp_files.load(Ordering::Relaxed),
                                total_files,
                                completed_bytes: bytes_done,
                                total_bytes: total_bytes.max(bytes_done),
                                current_file: file_name.clone(),
                            });
                        }
                    }
                };

                let res = dm
                    .download_single_inner(&task, Some(&comp_bytes), Some(&cancel), Some(&chunk_cb))
                    .await;

                match res {
                    Ok(was_downloaded) => {
                        let files_done = comp_files.fetch_add(1, Ordering::Relaxed) + 1;
                        let now = now_ms();
                        let prev = emit_ms.load(Ordering::Relaxed);
                        let should_emit = was_downloaded
                            || files_done == total_files
                            || (now.saturating_sub(prev) >= 80
                                && emit_ms
                                    .compare_exchange(
                                        prev,
                                        now,
                                        Ordering::Relaxed,
                                        Ordering::Relaxed,
                                    )
                                    .is_ok());

                        if should_emit {
                            if was_downloaded {
                                emit_ms.store(now, Ordering::Relaxed);
                            }
                            let bytes_done = comp_bytes.load(Ordering::Relaxed);
                            cb(DownloadProgress {
                                completed_files: files_done,
                                total_files,
                                completed_bytes: bytes_done,
                                total_bytes: total_bytes.max(bytes_done),
                                current_file: file_name,
                            });
                        }
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            });
        }

        loop {
            let next = tokio::select! {
                biased;
                _ = wait_cancelled(Some(&cancelled)) => {
                    join_set.shutdown().await;
                    return Err(LauncherErrorPayload::new(
                        launcher_types::ErrorCode::Cancelled,
                        "Download dibatalkan oleh pengguna.",
                    ));
                }
                res = join_set.join_next() => res,
            };
            let Some(res) = next else {
                break;
            };
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    join_set.shutdown().await;
                    return Err(e);
                }
                Err(join_err) => {
                    join_set.shutdown().await;
                    return Err(LauncherErrorPayload::internal(format!(
                        "Task download task panic / gagal di-join: {}",
                        join_err
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_verify_local_file_success() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("sample.txt");
        fs::write(&file_path, b"Minecraft Launcher Rust")
            .await
            .unwrap();

        let hash = hex::encode(Sha1::digest(b"Minecraft Launcher Rust"));

        assert!(verify_local_file(&file_path, Some(23), Some(&hash)).await);
        assert!(!verify_local_file(&file_path, Some(10), Some(&hash)).await);
        assert!(
            !verify_local_file(
                &file_path,
                Some(23),
                Some("0000000000000000000000000000000000000000")
            )
            .await
        );
    }

    #[tokio::test]
    async fn test_download_real_file() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("morechathistory.jar");
        let dm = DownloadManager::new(4);
        let task = DownloadTask {
            url:
                "https://cdn.modrinth.com/data/8qkXwOnk/versions/qpDhYv1Y/morechathistory-2.0.0.jar"
                    .to_string(),
            destination: file_path.clone(),
            expected_sha1: Some("6ce97845f93090dd7556b6e64bb7d63e126b83b4".to_string()),
            expected_sha256: None,
            expected_size: Some(3773),
        };
        let res = dm.download_single(&task, None, None).await;
        assert!(res.is_ok(), "download_single failed: {:?}", res);
        assert!(file_path.exists());
    }

    #[test]
    fn test_temp_file_guard_cleanup_on_drop_bug15() {
        let temp = tempdir().unwrap();
        let temp_file = temp.path().join("download.tmp-test1");
        std::fs::write(&temp_file, b"in-flight aborted data").unwrap();
        assert!(temp_file.exists());

        {
            let _guard = TempFileGuard::new(temp_file.clone());
        }
        assert!(
            !temp_file.exists(),
            "TempFileGuard must delete in-flight temporary file on drop"
        );
    }

    #[test]
    fn test_temp_file_guard_disarmed_preserves_file_bug15() {
        let temp = tempdir().unwrap();
        let temp_file = temp.path().join("download.tmp-test2");
        std::fs::write(&temp_file, b"final data").unwrap();

        {
            let mut guard = TempFileGuard::new(temp_file.clone());
            guard.disarm();
        }
        assert!(
            temp_file.exists(),
            "Disarmed TempFileGuard must keep finalized file on disk"
        );
    }

    #[tokio::test]
    async fn test_n20_verify_local_file_empty_sha1_consistent_with_post_download() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("sample.txt");
        fs::write(&file_path, b"Minecraft Launcher Rust")
            .await
            .unwrap();

        assert!(
            !verify_local_file(&file_path, Some(23), Some("")).await,
            "Some(\"\") expected_sha1 must fail local cache check just like post-download verification"
        );
    }

    #[tokio::test]
    async fn test_n20_download_single_checks_sha256_on_cached_file() {
        use tokio::io::AsyncWriteExt;

        let temp = tempdir().unwrap();
        let dest = temp.path().join("mod.jar");
        let bad_bytes = b"wrongdata!";
        let good_bytes = b"good_data!";
        assert_eq!(bad_bytes.len(), good_bytes.len());
        fs::write(&dest, bad_bytes).await.unwrap();

        let expected_sha256 = hex::encode(Sha256::digest(good_bytes));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    good_bytes.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.write_all(good_bytes).await;
                let _ = stream.shutdown().await;
            }
        });

        let dm = DownloadManager::new_loopback_test(1);
        let task = DownloadTask {
            url: format!("http://{}/mod.jar", addr),
            destination: dest.clone(),
            expected_sha1: None,
            expected_sha256: Some(expected_sha256),
            expected_size: Some(good_bytes.len() as u64),
        };

        dm.download_single(&task, None, None).await.unwrap();
        server.abort();
        assert_eq!(
            fs::read(&dest).await.unwrap(),
            good_bytes,
            "download_single must not trust stale cached file with mismatching expected_sha256"
        );
    }

    #[tokio::test]
    async fn test_n20_atomic_rename_preserves_existing_on_failure() {
        let temp = tempdir().unwrap();
        let missing_tmp = temp.path().join("missing.tmp");
        let dest = temp.path().join("existing.jar");
        fs::write(&dest, b"keep me intact").await.unwrap();

        let res = replace_destination(&missing_tmp, &dest).await;
        assert!(res.is_err());
        assert!(
            dest.exists(),
            "existing destination must not be removed before rename succeeds"
        );
    }

    #[tokio::test]
    async fn test_h6_download_single_rejects_truncated_payload_without_hash() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let mut req = [0u8; 512];
                    let _ = stream.read(&mut req).await;
                    let partial = b"abc";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        partial.len()
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                    let _ = stream.write_all(partial).await;
                    let _ = stream.shutdown().await;
                }
            }
        });

        let temp = tempdir().unwrap();
        let dest = temp.path().join("truncated.bin");
        let dm = DownloadManager::new_loopback_test(1);
        let task = DownloadTask {
            url: format!("http://{}/truncated.bin", addr),
            destination: dest.clone(),
            expected_sha1: None,
            expected_sha256: None,
            expected_size: Some(64),
        };

        let res = dm.download_single(&task, None, None).await;
        server.abort();
        assert!(res.is_err());
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn test_m7_cancelled_download_returns_error_code_cancelled() {
        let temp = tempdir().unwrap();
        let dest = temp.path().join("cancelled.bin");
        let dm = DownloadManager::new_loopback_test(1);
        let task = DownloadTask {
            url: "https://example.com/file.bin".to_string(),
            destination: dest,
            expected_sha1: None,
            expected_sha256: None,
            expected_size: None,
        };
        let cancel = Arc::new(AtomicBool::new(true));
        let err = dm
            .download_single(&task, None, Some(&cancel))
            .await
            .expect_err("Pre-cancelled download must fail");
        assert_eq!(err.code, launcher_types::ErrorCode::Cancelled);
    }

    #[tokio::test]
    async fn test_c1_download_all_deduplicates_destinations_and_rejects_conflicts() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let request_count = Arc::new(AtomicUsize::new(0));
        let req_count_srv = Arc::clone(&request_count);
        let payload = b"shared-asset-bytes".to_vec();
        let expected_sha1 = hex::encode(Sha1::digest(&payload));

        let server = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                req_count_srv.fetch_add(1, Ordering::Relaxed);
                let mut req = [0u8; 512];
                let _ = stream.read(&mut req).await;
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(&payload).await;
                let _ = stream.shutdown().await;
            }
        });

        let temp = tempdir().unwrap();
        let dest = temp.path().join("objects/ab/ab1234");
        let tasks: Vec<DownloadTask> = (0..8)
            .map(|_| DownloadTask {
                url: format!("http://{}/ab1234", addr),
                destination: dest.clone(),
                expected_sha1: Some(expected_sha1.clone()),
                expected_sha256: None,
                expected_size: Some(18),
            })
            .collect();

        let dm = DownloadManager::new_loopback_test(8);
        let max_total = Arc::new(AtomicUsize::new(0));
        let max_total_cb = Arc::clone(&max_total);
        dm.download_all(
            tasks,
            move |p| {
                max_total_cb.store(p.total_files, Ordering::Relaxed);
            },
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("Deduplicated duplicate destinations must succeed");
        server.abort();

        assert_eq!(request_count.load(Ordering::Relaxed), 1);
        assert_eq!(max_total.load(Ordering::Relaxed), 1);

        let conflicting = vec![
            DownloadTask {
                url: "https://example.com/a".to_string(),
                destination: dest.clone(),
                expected_sha1: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
                expected_sha256: None,
                expected_size: Some(10),
            },
            DownloadTask {
                url: "https://example.com/b".to_string(),
                destination: dest,
                expected_sha1: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
                expected_sha256: None,
                expected_size: Some(10),
            },
        ];
        let conflict_err = dm
            .download_all(conflicting, |_| {}, Arc::new(AtomicBool::new(false)))
            .await
            .expect_err("Conflicting hashes for same destination must be rejected");
        assert_eq!(conflict_err.code, launcher_types::ErrorCode::InvalidInput);
    }

    #[tokio::test]
    async fn test_h11_hash_mismatch_returns_hash_mismatch_and_cancel_aborts_stream() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let bad_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bad_addr = bad_listener.local_addr().unwrap();
        let bad_server = tokio::spawn(async move {
            for _ in 0..2 {
                if let Ok((mut stream, _)) = bad_listener.accept().await {
                    let mut req = [0u8; 512];
                    let _ = stream.read(&mut req).await;
                    let body = b"corrupted";
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes()).await;
                    let _ = stream.write_all(body).await;
                    let _ = stream.shutdown().await;
                }
            }
        });

        let temp = tempdir().unwrap();
        let dm = DownloadManager::new_loopback_test(1);
        let mismatch_err = dm
            .download_single(
                &DownloadTask {
                    url: format!("http://{}/bad.bin", bad_addr),
                    destination: temp.path().join("bad.bin"),
                    expected_sha1: Some("0000000000000000000000000000000000000000".to_string()),
                    expected_sha256: None,
                    expected_size: Some(9),
                },
                None,
                None,
            )
            .await
            .expect_err("Hash mismatch must fail");
        bad_server.abort();
        assert_eq!(mismatch_err.code, launcher_types::ErrorCode::HashMismatch);

        let slow_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let slow_addr = slow_listener.local_addr().unwrap();
        let slow_server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = slow_listener.accept().await {
                let mut req = [0u8; 512];
                let _ = stream.read(&mut req).await;
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1024\r\n\r\nhello")
                    .await;
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_trigger = Arc::clone(&cancel);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            cancel_trigger.store(true, Ordering::Relaxed);
        });

        let started = std::time::Instant::now();
        let cancel_err = dm
            .download_single(
                &DownloadTask {
                    url: format!("http://{}/slow.bin", slow_addr),
                    destination: temp.path().join("slow.bin"),
                    expected_sha1: None,
                    expected_sha256: None,
                    expected_size: None,
                },
                None,
                Some(&cancel),
            )
            .await
            .expect_err("Active stream cancel must abort promptly");
        slow_server.abort();
        assert_eq!(cancel_err.code, launcher_types::ErrorCode::Cancelled);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
