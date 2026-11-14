use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

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
        if size > 0 {
            if let Ok(meta) = fs::metadata(path).await {
                if meta.len() != size {
                    return false;
                }
            } else {
                return false;
            }
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
        if let Some(c) = cancelled {
            if c.load(Ordering::Relaxed) {
                return Err(LauncherErrorPayload::internal(
                    "Download dibatalkan oleh pengguna.",
                ));
            }
        }

        if (task.expected_sha1.is_some() || task.expected_sha256.is_some())
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
            return Ok(());
        }

        if let Some(parent) = task.destination.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                LauncherErrorPayload::file_system(format!(
                    "Gagal membuat folder {:?}: {}",
                    parent, e
                ))
            })?;
        }

        let temp_filename = format!(
            "{}.tmp-{}",
            task.destination
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("download"),
            uuid::Uuid::new_v4().simple()
        );
        let temp_path = task
            .destination
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(temp_filename);
        let mut temp_guard = TempFileGuard::new(temp_path.clone());

        let response = self.http.get(&task.url).await?;
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
            .filter(|&s| s > 0)
            .map(|s| s.min(MAX_DOWNLOAD_BYTES))
            .unwrap_or(MAX_DOWNLOAD_BYTES);

        let write_res: Result<(), LauncherErrorPayload> = async {
            while let Some(chunk_result) = stream.next().await {
                if let Some(c) = cancelled {
                    if c.load(Ordering::Relaxed) {
                        return Err(LauncherErrorPayload::internal(
                            "Download dibatalkan oleh pengguna.",
                        ));
                    }
                }

                let chunk = chunk_result.map_err(|e| {
                    LauncherErrorPayload::network(format!(
                        "Koneksi terputus saat mengunduh {}: {}",
                        task.url, e
                    ))
                })?;

                let chunk_len = chunk.len() as u64;
                downloaded_bytes = downloaded_bytes.saturating_add(chunk_len);
                if downloaded_bytes > max_allowed_bytes {
                    return Err(LauncherErrorPayload::network(format!(
                        "Ukuran unduhan melebihi batas yang diizinkan untuk {}",
                        task.url
                    )));
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

        if let Err(e) = write_res {
            let _ = fs::remove_file(&temp_path).await;
            return Err(e);
        }
        drop(file);

        if let Some(ref expected_sha1) = task.expected_sha1 {
            let actual_sha1 = hex::encode(sha1_hasher.finalize());
            if !actual_sha1.eq_ignore_ascii_case(expected_sha1) {
                let _ = fs::remove_file(&temp_path).await;
                return Err(LauncherErrorPayload::network(format!(
                    "Integritas SHA-1 tidak cocok untuk {}. Diharapkan: {}, Didapat: {}",
                    task.url, expected_sha1, actual_sha1
                )));
            }
        }

        if let Some(ref expected_sha256) = task.expected_sha256 {
            let actual_sha256 = hex::encode(sha256_hasher.finalize());
            if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
                let _ = fs::remove_file(&temp_path).await;
                return Err(LauncherErrorPayload::network(format!(
                    "Integritas SHA-256 tidak cocok untuk {}. Diharapkan: {}, Didapat: {}",
                    task.url, expected_sha256, actual_sha256
                )));
            }
        }

        replace_destination(&temp_path, &task.destination).await?;

        temp_guard.disarm();
        Ok(())
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
        let total_files = tasks.len();
        if total_files == 0 {
            return Ok(());
        }

        let total_bytes: u64 = tasks.iter().filter_map(|t| t.expected_size).sum();
        let completed_files = Arc::new(AtomicUsize::new(0));
        let completed_bytes = Arc::new(AtomicU64::new(0));
        let semaphore = Arc::new(Semaphore::new(self.parallelism()));
        let callback = Arc::new(progress_callback);

        let mut join_set = tokio::task::JoinSet::new();

        for task in tasks {
            let sem = Arc::clone(&semaphore);
            let comp_files = Arc::clone(&completed_files);
            let comp_bytes = Arc::clone(&completed_bytes);
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

                let res = dm
                    .download_single(&task, Some(&comp_bytes), Some(&cancel))
                    .await;

                if res.is_ok() {
                    let files_done = comp_files.fetch_add(1, Ordering::Relaxed) + 1;
                    let bytes_done = comp_bytes.load(Ordering::Relaxed);
                    cb(DownloadProgress {
                        completed_files: files_done,
                        total_files,
                        completed_bytes: bytes_done,
                        total_bytes: total_bytes.max(bytes_done),
                        current_file: file_name,
                    });
                }

                res
            });
        }

        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(download_res) => download_res?,
                Err(join_err) => {
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

        let dm = DownloadManager::new(1);
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
}
