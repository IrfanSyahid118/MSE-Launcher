use launcher_types::LauncherErrorPayload;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

#[cfg(target_os = "windows")]
pub const DEFAULT_USER_AGENT: &str = concat!(
    "MSELauncher/",
    env!("CARGO_PKG_VERSION"),
    " (Windows; x64; +https://github.com/IrfanSyahid118/MSE-Launcher)"
);

#[cfg(target_os = "linux")]
pub const DEFAULT_USER_AGENT: &str = concat!(
    "MSELauncher/",
    env!("CARGO_PKG_VERSION"),
    " (Linux; x64; +https://github.com/IrfanSyahid118/MSE-Launcher)"
);

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub const DEFAULT_USER_AGENT: &str = concat!(
    "MSELauncher/",
    env!("CARGO_PKG_VERSION"),
    " (Desktop; +https://github.com/IrfanSyahid118/MSE-Launcher)"
);

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

struct SafeDnsResolver;

impl Resolve for SafeDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            #[cfg(not(test))]
            if HttpClient::is_private_or_loopback_host(&host) {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Host privat/internal ditolak oleh kebijakan SSRF: {host}"),
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let addrs: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();

            #[cfg(not(test))]
            if addrs
                .iter()
                .any(|addr| HttpClient::is_disallowed_ip(addr.ip()))
            {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Resolusi DNS ke IP privat/loopback ditolak: {host}"),
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let iter: Addrs = Box::new(addrs.into_iter());
            Ok(iter)
        })
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    retry_config: RetryConfig,
    allow_loopback_http: bool,
}

impl HttpClient {
    fn is_safe_redirect_target_inner(url: &reqwest::Url) -> bool {
        match url.scheme() {
            "https" => {
                #[cfg(not(test))]
                if let Some(host) = url.host_str() {
                    if Self::is_private_or_loopback_host(host) {
                        return false;
                    }
                }
                true
            }
            #[cfg(test)]
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
            .dns_resolver(Arc::new(SafeDnsResolver))
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
            allow_loopback_http: false,
        }
    }

    #[cfg(any(test, feature = "test-http"))]
    pub fn new_loopback_test() -> Self {
        let mut client = Self::new();
        client.allow_loopback_http = true;
        client
    }

    #[cfg(test)]
    pub fn inner(&self) -> &Client {
        &self.client
    }

    #[cfg(test)]
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn request(
        &self,
        method: Method,
        url: &str,
    ) -> Result<RequestBuilder, LauncherErrorPayload> {
        if !self.is_allowed_url_for_client(url) {
            return Err(LauncherErrorPayload::invalid_input(
                "Hanya protokol HTTPS yang diizinkan untuk permintaan jaringan eksternal (SEC-06).",
            ));
        }
        Ok(self.client.request(method, url))
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
        // Walk Error::source() so nested rustls/webpki handshake and cert failures are not hidden by reqwest's Display.
        let mut cursor: Option<&(dyn std::error::Error + 'static)> = Some(e);
        while let Some(err) = cursor {
            let text = format!("{err} {err:?}").to_ascii_lowercase();
            if text.contains("certificate")
                || text.contains("unknownissuer")
                || text.contains("badcert")
                || text.contains("cert_")
                || text.contains("invalid peer")
                || text.contains("self signed")
                || text.contains("corruptmessage")
                || text.contains("corrupt message")
                || text.contains("invalidcontenttype")
                || text.contains("invalid content type")
                || text.contains("invalidmessage")
                || text.contains("invalid message")
                || text.contains("handshakefailure")
                || text.contains("handshake failure")
                || text.contains("alertreceived")
                || text.contains("alert received")
                || text.contains("unknownprotocol")
                || text.contains("unknown protocol")
                || text.contains("invalidpeer")
                || text.contains("tls handshake")
            {
                return false;
            }
            cursor = err.source();
        }
        true
    }

    fn calculate_delay(&self, attempt: u32) -> Duration {
        let factor = self.retry_config.backoff_factor.powi(attempt as i32);
        let millis = (self.retry_config.initial_delay.as_millis() as f64 * factor) as u64;
        let jitter = fastrand::u64(0..=250);
        let max_ms = self
            .retry_config
            .max_delay
            .as_millis()
            .min(u64::MAX as u128) as u64;
        let total_ms = millis.saturating_add(jitter).min(max_ms);
        Duration::from_millis(total_ms)
    }

    fn is_disallowed_ipv4(v4: Ipv4Addr) -> bool {
        v4.is_loopback()
            || v4.is_private()
            || v4.is_link_local()
            || v4.is_unspecified()
            || v4.is_broadcast()
            || v4.is_multicast()
            || v4.is_documentation()
            || (v4.octets()[0] == 100 && (v4.octets()[1] & 0b1100_0000) == 64)
    }

    fn is_disallowed_ip(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => Self::is_disallowed_ipv4(v4),
            IpAddr::V6(v6) => {
                if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                    return true;
                }
                if let Some(mapped) = v6.to_ipv4_mapped() {
                    return Self::is_disallowed_ipv4(mapped);
                }
                if let Some(compat) = v6.to_ipv4() {
                    return Self::is_disallowed_ipv4(compat);
                }
                let seg = v6.segments();
                // NAT64 well-known prefix 64:ff9b::/96 embeds IPv4 in the final 32 bits.
                if seg[0] == 0x0064 && seg[1] == 0xff9b && seg[2..6] == [0, 0, 0, 0] {
                    let embedded = Ipv4Addr::from(((seg[6] as u32) << 16) | (seg[7] as u32));
                    return Self::is_disallowed_ipv4(embedded);
                }
                (seg[0] & 0xffc0) == 0xfe80
                    || (seg[0] & 0xfe00) == 0xfc00
                    || (seg[0] == 0x2001 && seg[1] == 0x0db8)
            }
        }
    }

    pub fn is_private_or_loopback_host(host: &str) -> bool {
        let bare = host
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if bare == "localhost"
            || bare.ends_with(".localhost")
            || bare.ends_with(".local")
            || bare.ends_with(".internal")
        {
            return true;
        }
        if let Ok(ip) = bare.parse::<IpAddr>() {
            return Self::is_disallowed_ip(ip);
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
                #[cfg(not(test))]
                if Self::is_private_or_loopback_host(host) {
                    return false;
                }
                let _ = host;
                true
            }
            #[cfg(test)]
            "http" => matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
            _ => false,
        }
    }

    fn is_allowed_url_for_client(&self, url: &str) -> bool {
        if Self::is_allowed_request_url(url) {
            return true;
        }
        if self.allow_loopback_http {
            if let Ok(parsed) = reqwest::Url::parse(url) {
                return parsed.scheme() == "http"
                    && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
            }
        }
        false
    }

    pub async fn get(&self, url: &str) -> Result<Response, LauncherErrorPayload> {
        if !self.is_allowed_url_for_client(url) {
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
                        let delay = self.calculate_delay(attempt);
                        attempt += 1;
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
                        let delay = self.calculate_delay(attempt);
                        attempt += 1;
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

        let server = tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 512];
                let _ = socket.read(&mut buf).await;
                let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await;
                let _ = socket.shutdown().await;
                // Keep socket alive briefly so Windows sends clean TLS bytes before TCP close.
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });

        let client = HttpClient::new();
        let started = std::time::Instant::now();
        let res = client.get(&format!("https://127.0.0.1:{}", port)).await;
        server.abort();
        assert!(res.is_err(), "HTTPS connection to non-TLS server must fail");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "TLS failure must not trigger retry backoff, elapsed={:?}, res={:?}",
            started.elapsed(),
            res
        );
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
        assert!(HttpClient::is_private_or_loopback_host("localhost."));
        assert!(HttpClient::is_private_or_loopback_host(
            "metadata.google.internal."
        ));
        assert!(HttpClient::is_private_or_loopback_host("[::127.0.0.1]"));
        assert!(HttpClient::is_private_or_loopback_host(
            "[::ffff:100.64.0.1]"
        ));
        assert!(HttpClient::is_private_or_loopback_host(
            "[64:ff9b::127.0.0.1]"
        ));
        assert!(!HttpClient::is_private_or_loopback_host("api.modrinth.com"));
        assert!(!HttpClient::is_private_or_loopback_host("8.8.8.8"));
    }
}
