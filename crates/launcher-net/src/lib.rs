use launcher_types::LauncherErrorPayload;
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;

#[cfg(target_os = "windows")]
pub const DEFAULT_USER_AGENT: &str =
    "MSELauncher/1.0.0-beta (Windows; x64; +https://github.com/IrfanSyahid118/MSE-Launcher)";

#[cfg(target_os = "linux")]
pub const DEFAULT_USER_AGENT: &str =
    "MSELauncher/1.0.0-beta (Linux; x64; +https://github.com/IrfanSyahid118/MSE-Launcher)";

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub const DEFAULT_USER_AGENT: &str =
    "MSELauncher/1.0.0-beta (Desktop; +https://github.com/IrfanSyahid118/MSE-Launcher)";

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            backoff_factor: 2.0,
        }
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    retry_config: RetryConfig,
}

impl HttpClient {
    fn is_safe_redirect_target_inner(url: &reqwest::Url) -> bool {
        match url.scheme() {
            "https" => {
                #[cfg(not(any(test, debug_assertions)))]
                if let Some(host) = url.host_str() {
                    if Self::is_private_or_loopback_host(host) {
                        return false;
                    }
                }
                true
            }
            #[cfg(any(test, debug_assertions))]
            "http" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
            _ => false,
        }
    }

    pub fn new() -> Self {
        Self::with_config(RetryConfig::default())
    }

    pub fn with_config(retry_config: RetryConfig) -> Self {
        let client = Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().iter().any(|u| u.scheme() == "https")
                    && attempt.url().scheme() == "http"
                {
                    attempt.error("Downgrade redirect dari HTTPS ke HTTP dilarang (SEC-06)")
                } else if !Self::is_safe_redirect_target_inner(attempt.url()) {
                    attempt.error("Redirect ke host internal atau skema tidak diizinkan (SEC-06)")
                } else if attempt.previous().len() >= 10 {
                    attempt.error("Terlalu banyak redirect HTTP")
                } else {
                    attempt.follow()
                }
            }))
            .gzip(false)
            .build()
            .expect("Gagal menginisialisasi reqwest Client");

        Self {
            client,
            retry_config,
        }
    }

    pub fn inner(&self) -> &Client {
        &self.client
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn should_retry(status: StatusCode) -> bool {
        matches!(
            status,
            StatusCode::REQUEST_TIMEOUT
                | StatusCode::TOO_MANY_REQUESTS
                | StatusCode::INTERNAL_SERVER_ERROR
                | StatusCode::BAD_GATEWAY
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::GATEWAY_TIMEOUT
        )
    }

    pub fn is_transient_error(e: &reqwest::Error) -> bool {
        if e.is_redirect() || e.is_builder() || e.is_decode() {
            return false;
        }
        let err_str = e.to_string().to_lowercase();
        !(err_str.contains("certificate")
            || err_str.contains("unknownissuer")
            || err_str.contains("badcert")
            || err_str.contains("cert_")
            || err_str.contains("invalid peer certificate")
            || err_str.contains("self signed"))
    }

    fn calculate_delay(&self, attempt: u32) -> Duration {
        let factor = self.retry_config.backoff_factor.powi(attempt as i32);
        let millis = (self.retry_config.initial_delay.as_millis() as f64 * factor) as u64;
        let jitter = fastrand::u64(0..=250);
        let total_ms = (millis + jitter).min(self.retry_config.max_delay.as_millis() as u64);
        Duration::from_millis(total_ms)
    }

    pub fn is_private_or_loopback_host(host: &str) -> bool {
        let bare = host
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_ascii_lowercase();
        if bare == "localhost"
            || bare.ends_with(".localhost")
            || bare.ends_with(".local")
            || bare.ends_with(".internal")
        {
            return true;
        }
        if let Ok(ip) = bare.parse::<std::net::IpAddr>() {
            return match ip {
                std::net::IpAddr::V4(v4) => {
                    v4.is_loopback()
                        || v4.is_private()
                        || v4.is_link_local()
                        || v4.is_unspecified()
                        || v4.is_broadcast()
                        || v4.is_multicast()
                        || (v4.octets()[0] == 100 && (v4.octets()[1] & 0b1100_0000) == 64)
                }
                std::net::IpAddr::V6(v6) => {
                    if let Some(mapped) = v6.to_ipv4_mapped() {
                        return mapped.is_loopback()
                            || mapped.is_private()
                            || mapped.is_link_local()
                            || mapped.is_unspecified()
                            || mapped.is_broadcast()
                            || mapped.is_multicast();
                    }
                    let seg0 = v6.segments()[0];
                    v6.is_loopback()
                        || v6.is_unspecified()
                        || v6.is_multicast()
                        || (seg0 & 0xffc0) == 0xfe80
                        || (seg0 & 0xfe00) == 0xfc00
                }
            };
        }
        false
    }

    pub fn is_allowed_request_url(url: &str) -> bool {
        let Ok(parsed) = reqwest::Url::parse(url) else {
            return false;
        };
        let host = parsed.host_str().unwrap_or_default();
        match parsed.scheme() {
            "https" => {
                #[cfg(not(any(test, debug_assertions)))]
                if Self::is_private_or_loopback_host(host) {
                    return false;
                }
                let _ = host;
                true
            }
            #[cfg(any(test, debug_assertions))]
            "http" => matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
            _ => false,
        }
    }

    pub async fn get(&self, url: &str) -> Result<Response, LauncherErrorPayload> {
        if !Self::is_allowed_request_url(url) {
            return Err(LauncherErrorPayload::invalid_input(
                "Hanya protokol HTTPS yang diizinkan untuk permintaan jaringan eksternal (SEC-06).",
            ));
        }

        let mut attempt = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp);
                    }

                    if Self::should_retry(status) && attempt < self.retry_config.max_retries {
                        attempt += 1;
                        let delay = self.calculate_delay(attempt);
                        tracing::warn!(
                            "Permintaan ke {} gagal dengan status {}. Percobaan ulang {}/{} dalam {:?}",
                            url,
                            status,
                            attempt,
                            self.retry_config.max_retries,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Err(LauncherErrorPayload::network(format!(
                        "Permintaan HTTP ke {} gagal dengan status HTTP {}",
                        url, status
                    )));
                }
                Err(e) => {
                    let is_transient = Self::is_transient_error(&e);
                    if is_transient && attempt < self.retry_config.max_retries {
                        attempt += 1;
                        let delay = self.calculate_delay(attempt);
                        tracing::warn!(
                            "Koneksi ke {} terputus ({:?}). Percobaan ulang {}/{} dalam {:?}",
                            url,
                            e,
                            attempt,
                            self.retry_config.max_retries,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Err(LauncherErrorPayload::network(format!(
                        "Gagal menghubungi {}: {}",
                        url, e
                    )));
                }
            }
        }
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<T, LauncherErrorPayload> {
        let resp = self.get(url).await?;
        resp.json::<T>().await.map_err(|e| {
            LauncherErrorPayload::network(format!(
                "Gagal mem-parse respon JSON dari {}: {}",
                url, e
            ))
        })
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_status_check() {
        assert!(HttpClient::should_retry(StatusCode::TOO_MANY_REQUESTS));
        assert!(HttpClient::should_retry(StatusCode::BAD_GATEWAY));
        assert!(HttpClient::should_retry(StatusCode::SERVICE_UNAVAILABLE));
        assert!(!HttpClient::should_retry(StatusCode::NOT_FOUND));
        assert!(!HttpClient::should_retry(StatusCode::BAD_REQUEST));
        assert!(!HttpClient::should_retry(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn test_calculate_delay_jitter() {
        let client = HttpClient::new();
        let delay1 = client.calculate_delay(1);
        let delay2 = client.calculate_delay(1);
        assert!(delay1.as_millis() >= 1000);
        assert!(delay2.as_millis() >= 1000);
        assert!(delay1.as_millis() <= 1500);
    }

    #[tokio::test]
    async fn test_tls_bad_cert_not_retried_bug12() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                use tokio::io::AsyncWriteExt;
                let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await;
            }
        });

        let client = HttpClient::new();
        let res = client.get(&format!("https://127.0.0.1:{}", port)).await;
        assert!(res.is_err(), "HTTPS connection to non-TLS server must fail");
    }

    #[test]
    fn test_n21_rejects_spoofed_localhost_domains() {
        assert!(HttpClient::is_allowed_request_url(
            "https://api.modrinth.com/v2/search"
        ));
        assert!(HttpClient::is_allowed_request_url(
            "http://localhost:8080/health"
        ));
        assert!(HttpClient::is_allowed_request_url(
            "http://127.0.0.1:1234/file"
        ));
        assert!(HttpClient::is_allowed_request_url("http://[::1]:1234/file"));

        assert!(
            !HttpClient::is_allowed_request_url("http://localhost.evil.com/steal"),
            "http://localhost.evil.com must be rejected by HTTPS-only check"
        );
        assert!(
            !HttpClient::is_allowed_request_url("http://127.0.0.1.evil.com/steal"),
            "http://127.0.0.1.evil.com must be rejected by HTTPS-only check"
        );
        assert!(
            !HttpClient::is_allowed_request_url("http://localhost@evil.com/steal"),
            "http://localhost@evil.com must be rejected by HTTPS-only check"
        );
    }

    #[test]
    fn test_ssrf_private_and_loopback_host_detection() {
        assert!(HttpClient::is_private_or_loopback_host("127.0.0.1"));
        assert!(HttpClient::is_private_or_loopback_host("10.0.0.5"));
        assert!(HttpClient::is_private_or_loopback_host("172.16.4.2"));
        assert!(HttpClient::is_private_or_loopback_host("192.168.1.1"));
        assert!(HttpClient::is_private_or_loopback_host("169.254.169.254"));
        assert!(HttpClient::is_private_or_loopback_host("localhost"));
        assert!(HttpClient::is_private_or_loopback_host("[::1]"));
        assert!(HttpClient::is_private_or_loopback_host("fe80::1"));
        assert!(!HttpClient::is_private_or_loopback_host("api.modrinth.com"));
        assert!(!HttpClient::is_private_or_loopback_host("8.8.8.8"));
    }
}
