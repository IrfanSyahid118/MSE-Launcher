use chrono::Utc;
use launcher_net::HttpClient;
use launcher_platform::{DataPaths, SecretStore};
use launcher_storage::AccountsStore;
use launcher_types::{Account, AccountTokens, AccountType, DeviceCodeInfo, LauncherErrorPayload};
use reqwest::Method;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use zeroize::Zeroize;

const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBOX_USER_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT_LOGIN_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XboxClaim {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XboxDisplayClaims {
    xui: Vec<XboxClaim>,
}

#[derive(Debug, Deserialize)]
struct XboxUserResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XboxDisplayClaims,
}

#[derive(Debug, Deserialize)]
struct XstsResponse {
    #[serde(rename = "Token")]
    token: String,
}

#[derive(Debug, Deserialize)]
struct MinecraftLoginResponse {
    access_token: String,
    expires_in: u64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MojangSkin {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    state: Option<String>,
    url: String,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    alias: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MojangCape {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    state: Option<String>,
    url: String,
    #[serde(default)]
    alias: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<MojangSkin>,
    #[serde(default)]
    capes: Vec<MojangCape>,
}

pub fn format_uuid(raw: &str) -> Result<String, LauncherErrorPayload> {
    let clean = raw.replace('-', "").to_lowercase();
    if clean.len() != 32 || !clean.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(LauncherErrorPayload::auth_failed(
            "Minecraft mengembalikan UUID profil yang tidak valid.",
        ));
    }
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &clean[0..8],
        &clean[8..12],
        &clean[12..16],
        &clean[16..20],
        &clean[20..32]
    ))
}

#[derive(Clone)]
pub struct MicrosoftAuthService {
    paths: DataPaths,
    http: HttpClient,
    accounts: AccountsStore,
    secrets: SecretStore,
    refresh_locks: Arc<std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl MicrosoftAuthService {
    pub fn new(paths: DataPaths, accounts: AccountsStore, secrets: SecretStore) -> Self {
        Self {
            paths,
            http: HttpClient::new(),
            accounts,
            secrets,
            refresh_locks: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_loopback_test(
        paths: DataPaths,
        accounts: AccountsStore,
        secrets: SecretStore,
    ) -> Self {
        Self {
            paths,
            http: HttpClient::new_loopback_test(),
            accounts,
            secrets,
            refresh_locks: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    fn refresh_lock_for(&self, account_id: &str) -> Arc<Mutex<()>> {
        let mut map = self
            .refresh_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.entry(account_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn is_expiry_still_valid(expires_at: Option<&str>) -> bool {
        let Some(exp_str) = expires_at else {
            return false;
        };
        let Ok(exp) = chrono::DateTime::parse_from_rfc3339(exp_str) else {
            return false;
        };
        exp.with_timezone(&Utc) > Utc::now() + chrono::Duration::minutes(5)
    }

    async fn sleep_or_cancel(
        dur: Duration,
        deadline: std::time::Instant,
        cancelled: Option<&Arc<AtomicBool>>,
    ) -> Result<(), LauncherErrorPayload> {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(LauncherErrorPayload::auth_failed(
                "Kode login kedaluwarsa. Mulai ulang proses masuk.",
            ));
        }
        let target = std::time::Instant::now() + dur.min(remaining);
        loop {
            if cancelled.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::Cancelled,
                    "Proses masuk Microsoft dibatalkan.",
                ));
            }
            let now = std::time::Instant::now();
            if now >= target {
                return Ok(());
            }
            sleep((target - now).min(Duration::from_millis(100))).await;
        }
    }

    pub fn client_id(&self) -> Option<String> {
        if let Ok(env_id) = std::env::var("MS_CLIENT_ID") {
            let trimmed = env_id.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        let local_config = self.paths.root.join("config.local.json");
        if local_config.exists() {
            if let Ok(contents) = fs::read_to_string(&local_config) {
                if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&contents) {
                    if let Some(id) = cfg.get("msClientId").and_then(|v| v.as_str()) {
                        let trimmed = id.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }

        let integrations = self.paths.root.join("integrations.json");
        if integrations.exists() {
            if let Ok(contents) = fs::read_to_string(&integrations) {
                if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&contents) {
                    if let Some(id) = cfg.get("msClientId").and_then(|v| v.as_str()) {
                        let trimmed = id.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }

        // Public Minecraft device-code client ID allowlisted by Mojang.
        Some("c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb".to_string())
    }

    pub async fn begin_login(&self) -> Result<DeviceCodeInfo, LauncherErrorPayload> {
        let client_id = self.client_id().ok_or_else(|| {
            LauncherErrorPayload::provider_not_configured(
                "Login Microsoft belum tersedia pada build launcher ini.",
            )
        })?;

        let res = self
            .http
            .request(Method::POST, DEVICE_CODE_URL)?
            .form(&[("client_id", client_id.as_str()), ("scope", SCOPE)])
            .send()
            .await
            .map_err(|e| {
                LauncherErrorPayload::network(format!("Gagal menghubungi server Microsoft: {}", e))
            })?;

        let status = res.status();
        let text = res.text().await.map_err(|e| {
            LauncherErrorPayload::network(format!(
                "Gagal membaca respons device code dari Microsoft: {}",
                e
            ))
        })?;

        if !status.is_success() {
            return Err(LauncherErrorPayload::auth_failed(format!(
                "Microsoft menolak permintaan kode login (HTTP {}).",
                status
            )));
        }

        let data: DeviceCodeResponse = serde_json::from_str(&text).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!(
                "Microsoft tidak memberikan kode login yang valid: {}",
                e
            ))
        })?;

        Ok(DeviceCodeInfo {
            device_code: data.device_code,
            user_code: data.user_code,
            verification_uri: data.verification_uri,
            expires_in: data.expires_in,
            interval: data.interval.unwrap_or(5),
        })
    }

    pub async fn complete_login(
        &self,
        code: DeviceCodeInfo,
    ) -> Result<Account, LauncherErrorPayload> {
        self.complete_login_with_cancel(code, None).await
    }

    pub async fn complete_login_with_cancel(
        &self,
        code: DeviceCodeInfo,
        cancelled: Option<Arc<AtomicBool>>,
    ) -> Result<Account, LauncherErrorPayload> {
        let client_id = self.client_id().ok_or_else(|| {
            LauncherErrorPayload::provider_not_configured("Client ID Microsoft tidak ditemukan.")
        })?;

        let capped_secs = code.expires_in.min(3600);
        let deadline = std::time::Instant::now()
            .checked_add(Duration::from_secs(capped_secs))
            .ok_or_else(|| {
                LauncherErrorPayload::invalid_input("Durasi kedaluwarsa kode login tidak valid.")
            })?;
        let mut interval_secs = code.interval.clamp(1, 60);

        let (mut access_token, mut refresh_token) = loop {
            if cancelled
                .as_ref()
                .is_some_and(|c| c.load(Ordering::Relaxed))
            {
                return Err(LauncherErrorPayload::new(
                    launcher_types::ErrorCode::Cancelled,
                    "Proses masuk Microsoft dibatalkan.",
                ));
            }
            if std::time::Instant::now() >= deadline {
                return Err(LauncherErrorPayload::auth_failed(
                    "Kode login kedaluwarsa. Mulai ulang proses masuk.",
                ));
            }

            let res = self
                .http
                .request(Method::POST, TOKEN_URL)?
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("client_id", client_id.as_str()),
                    ("device_code", code.device_code.as_str()),
                ])
                .send()
                .await
                .map_err(|e| {
                    LauncherErrorPayload::network(format!(
                        "Gagal menghubungi server Microsoft: {}",
                        e
                    ))
                })?;

            let status = res.status();
            let retry_after_secs = res
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(|s| s.clamp(1, 60));

            let mut text = res.text().await.map_err(|e| {
                LauncherErrorPayload::network(format!(
                    "Gagal membaca respons token server Microsoft: {}",
                    e
                ))
            })?;

            if status.as_u16() == 429 || status.is_server_error() {
                text.zeroize();
                let wait_secs = retry_after_secs.unwrap_or(interval_secs);
                Self::sleep_or_cancel(Duration::from_secs(wait_secs), deadline, cancelled.as_ref())
                    .await?;
                continue;
            }

            let body: TokenResponse = match serde_json::from_str(&text) {
                Ok(parsed) => {
                    text.zeroize();
                    parsed
                }
                Err(e) => {
                    text.zeroize();
                    return Err(LauncherErrorPayload::auth_failed(format!(
                        "Format respons token tidak valid (HTTP {}): {}",
                        status, e
                    )));
                }
            };

            if let Some(ref err) = body.error {
                match err.as_str() {
                    "authorization_pending" => {
                        Self::sleep_or_cancel(
                            Duration::from_secs(interval_secs),
                            deadline,
                            cancelled.as_ref(),
                        )
                        .await?;
                        continue;
                    }
                    "slow_down" => {
                        interval_secs = interval_secs.saturating_add(5).min(60);
                        Self::sleep_or_cancel(
                            Duration::from_secs(interval_secs),
                            deadline,
                            cancelled.as_ref(),
                        )
                        .await?;
                        continue;
                    }
                    "expired_token" | "code_expired" => {
                        return Err(LauncherErrorPayload::auth_failed(
                            "Kode login sudah kedaluwarsa. Mulai ulang proses masuk.",
                        ));
                    }
                    "authorization_declined" => {
                        return Err(LauncherErrorPayload::auth_failed(
                            "Login dibatalkan di browser.",
                        ));
                    }
                    other => {
                        return Err(LauncherErrorPayload::auth_failed(
                            body.error_description.unwrap_or_else(|| other.to_string()),
                        ));
                    }
                }
            }

            let access_token = body.access_token.ok_or_else(|| {
                LauncherErrorPayload::auth_failed(body.error_description.unwrap_or_else(|| {
                    "Token akses tidak ditemukan dalam respons Microsoft.".to_string()
                }))
            })?;

            break (access_token, body.refresh_token);
        };

        let result = self
            .exchange_and_save(access_token.clone(), refresh_token.clone(), None)
            .await;
        access_token.zeroize();
        if let Some(ref mut rt) = refresh_token {
            rt.zeroize();
        }
        result
    }

    pub async fn refresh_session(
        &self,
        account: &Account,
    ) -> Result<Account, LauncherErrorPayload> {
        let lock = self.refresh_lock_for(&account.id);
        let _guard = lock.lock().await;

        // If a concurrent caller just refreshed and persisted a non-expiring token, reuse it.
        if let Some(latest) = self.accounts.get(&account.id) {
            if Self::is_expiry_still_valid(latest.expires_at.as_deref()) {
                let secret_key = format!("tokens:{}", latest.id);
                if let Some(mut secret_data) = self.secrets.get(&secret_key).await {
                    let parsed = serde_json::from_str::<AccountTokens>(&secret_data);
                    secret_data.zeroize();
                    if let Ok(tokens) = parsed {
                        let mut hydrated = latest;
                        hydrated.tokens = Some(tokens);
                        return Ok(hydrated);
                    }
                }
            }
        }

        let client_id = self.client_id().ok_or_else(|| {
            LauncherErrorPayload::provider_not_configured("Client ID Microsoft tidak ditemukan.")
        })?;

        let secret_key = format!("tokens:{}", account.id);
        let mut stored_secret = self.secrets.get(&secret_key).await;
        let mut refresh_token = stored_secret
            .as_deref()
            .and_then(|data| {
                serde_json::from_str::<AccountTokens>(data)
                    .map(|t| t.refresh_token.clone())
                    .ok()
                    .or_else(|| Some(data.to_string()))
            })
            .or_else(|| account.tokens.as_ref().map(|t| t.refresh_token.clone()))
            .ok_or_else(|| {
                LauncherErrorPayload::auth_failed(
                    "Sesi akun Microsoft tidak ditemukan. Silakan masuk kembali.",
                )
            })?;
        if let Some(ref mut s) = stored_secret {
            s.zeroize();
        }

        let res = self
            .http
            .request(Method::POST, TOKEN_URL)?
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", client_id.as_str()),
                ("refresh_token", refresh_token.as_str()),
                ("scope", SCOPE),
            ])
            .send()
            .await
            .map_err(|e| {
                LauncherErrorPayload::network(format!("Gagal menghubungi server Microsoft: {}", e))
            })?;

        let status = res.status();
        let mut text = res.text().await.map_err(|e| {
            LauncherErrorPayload::network(format!("Gagal membaca respons server Microsoft: {}", e))
        })?;

        let token_resp: TokenResponse = match serde_json::from_str(&text) {
            Ok(parsed) => {
                text.zeroize();
                parsed
            }
            Err(e) => {
                text.zeroize();
                refresh_token.zeroize();
                return Err(LauncherErrorPayload::auth_failed(format!(
                    "Format respons refresh token tidak valid (HTTP {}): {}",
                    status, e
                )));
            }
        };

        if let Some(err) = token_resp.error {
            refresh_token.zeroize();
            return Err(LauncherErrorPayload::auth_failed(
                token_resp.error_description.unwrap_or(err),
            ));
        }

        let mut access_token = match token_resp.access_token {
            Some(tok) => tok,
            None => {
                refresh_token.zeroize();
                return Err(LauncherErrorPayload::auth_failed(
                    token_resp
                        .error_description
                        .unwrap_or_else(|| "Gagal memperbarui token Microsoft.".to_string()),
                ));
            }
        };

        let next_refresh = token_resp
            .refresh_token
            .or_else(|| Some(refresh_token.clone()));
        refresh_token.zeroize();

        let latest_account = self
            .accounts
            .get(&account.id)
            .unwrap_or_else(|| account.clone());
        let result = self
            .exchange_and_save(access_token.clone(), next_refresh, Some(&latest_account))
            .await;
        access_token.zeroize();
        result
    }

    async fn exchange_and_save(
        &self,
        ms_access_token: String,
        ms_refresh_token: Option<String>,
        existing_account: Option<&Account>,
    ) -> Result<Account, LauncherErrorPayload> {
        self.exchange_and_save_with_urls(
            ms_access_token,
            ms_refresh_token,
            existing_account,
            (
                XBOX_USER_URL,
                XSTS_URL,
                MINECRAFT_LOGIN_URL,
                MINECRAFT_PROFILE_URL,
            ),
        )
        .await
    }

    async fn exchange_and_save_with_urls(
        &self,
        ms_access_token: String,
        ms_refresh_token: Option<String>,
        existing_account: Option<&Account>,
        (xbox_user_url, xsts_url, mc_login_url, mc_profile_url): (&str, &str, &str, &str),
    ) -> Result<Account, LauncherErrorPayload> {
        let xbox_res = self
            .http
            .request(Method::POST, xbox_user_url)?
            .json(&serde_json::json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": format!("d={}", ms_access_token),
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT"
            }))
            .send()
            .await
            .map_err(|e| {
                LauncherErrorPayload::network(format!("Gagal menghubungi Xbox Live: {}", e))
            })?;

        let xbox_status = xbox_res.status();
        let mut xbox_text = xbox_res.text().await.unwrap_or_default();
        if !xbox_status.is_success() {
            xbox_text.zeroize();
            return Err(LauncherErrorPayload::auth_failed(format!(
                "Xbox Live menolak token Microsoft (HTTP {}).",
                xbox_status
            )));
        }

        let mut xbox_data: XboxUserResponse = match serde_json::from_str(&xbox_text) {
            Ok(data) => {
                xbox_text.zeroize();
                data
            }
            Err(e) => {
                xbox_text.zeroize();
                return Err(LauncherErrorPayload::auth_failed(format!(
                    "Format respons Xbox Live tidak valid: {}",
                    e
                )));
            }
        };

        let first_claim =
            xbox_data.display_claims.xui.first().ok_or_else(|| {
                LauncherErrorPayload::auth_failed("Klaim Xbox Live tidak ditemukan.")
            })?;
        let uhs = first_claim.uhs.clone();
        let xuid = first_claim.xid.clone();

        let xsts_res = self
            .http
            .request(Method::POST, xsts_url)?
            .json(&serde_json::json!({
                "Properties": {
                    "SandboxId": "RETAIL",
                    "UserTokens": [xbox_data.token]
                },
                "RelyingParty": "rp://api.minecraftservices.com/",
                "TokenType": "JWT"
            }))
            .send()
            .await
            .map_err(|e| LauncherErrorPayload::network(format!("Gagal menghubungi XSTS: {}", e)))?;
        xbox_data.token.zeroize();

        let xsts_status = xsts_res.status();
        let mut xsts_text = xsts_res.text().await.unwrap_or_default();
        if !xsts_status.is_success() {
            let err_msg = if xsts_text.contains("2148916238") {
                "Akun Microsoft ini merupakan akun anak di bawah umur yang memerlukan persetujuan orang tua (Xbox Family).".to_string()
            } else if xsts_text.contains("2148916233") {
                "Akun ini belum memiliki profil Xbox Live. Silakan buka xbox.com dan buat gamertag terlebih dahulu.".to_string()
            } else if xsts_text.contains("2148916235") {
                "Xbox Live belum tersedia di negara/wilayah akun Anda.".to_string()
            } else if xsts_text.contains("2148916236") || xsts_text.contains("2148916237") {
                "Akun ini memerlukan verifikasi usia dewasa (persyaratan regulasi setempat)."
                    .to_string()
            } else {
                format!("XSTS menolak token Xbox Live (HTTP {}).", xsts_status)
            };
            xsts_text.zeroize();
            return Err(LauncherErrorPayload::auth_failed(err_msg));
        }

        let mut xsts_data: XstsResponse = match serde_json::from_str(&xsts_text) {
            Ok(data) => {
                xsts_text.zeroize();
                data
            }
            Err(e) => {
                xsts_text.zeroize();
                return Err(LauncherErrorPayload::auth_failed(format!(
                    "Format respons XSTS tidak valid: {}",
                    e
                )));
            }
        };

        let mc_res = self
            .http
            .request(Method::POST, mc_login_url)?
            .json(&serde_json::json!({
                "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_data.token)
            }))
            .send()
            .await
            .map_err(|e| {
                LauncherErrorPayload::network(format!(
                    "Gagal menghubungi Minecraft Services: {}",
                    e
                ))
            })?;
        xsts_data.token.zeroize();

        let mc_status = mc_res.status();
        let mut mc_text = mc_res.text().await.unwrap_or_default();
        if !mc_status.is_success() {
            mc_text.zeroize();
            return Err(LauncherErrorPayload::auth_failed(format!(
                "Minecraft Services menolak login (HTTP {}).",
                mc_status
            )));
        }

        let mc_data: MinecraftLoginResponse = match serde_json::from_str(&mc_text) {
            Ok(data) => {
                mc_text.zeroize();
                data
            }
            Err(e) => {
                mc_text.zeroize();
                return Err(LauncherErrorPayload::auth_failed(format!(
                    "Format respons Minecraft Services tidak valid: {}",
                    e
                )));
            }
        };

        let profile_res = self
            .http
            .request(Method::GET, mc_profile_url)?
            .header("Authorization", format!("Bearer {}", mc_data.access_token))
            .send()
            .await
            .map_err(|e| {
                LauncherErrorPayload::network(format!("Gagal mengambil profil Minecraft: {}", e))
            })?;

        let status = profile_res.status();
        let profile_text = profile_res.text().await.unwrap_or_default();
        if !status.is_success() {
            if status.as_u16() == 404 {
                return Err(LauncherErrorPayload::auth_failed(
                    "Akun Microsoft ini belum memiliki lisensi atau profil Minecraft Java Edition resmi.",
                ));
            }
            return Err(LauncherErrorPayload::auth_failed(format!(
                "Gagal mengambil profil Minecraft (HTTP {}).",
                status
            )));
        }

        let profile_data: MinecraftProfileResponse =
            serde_json::from_str(&profile_text).map_err(|e| {
                LauncherErrorPayload::auth_failed(format!(
                    "Akun Minecraft tidak ditemukan atau belum memiliki lisensi Java Edition: {}",
                    e
                ))
            })?;

        let formatted_uuid = format_uuid(&profile_data.id)?;
        let dedup_existing = if existing_account.is_some() {
            None
        } else {
            self.accounts
                .list()
                .into_iter()
                .find(|a| a.account_type == AccountType::Microsoft && a.uuid == formatted_uuid)
        };
        let matched_account = existing_account.or(dedup_existing.as_ref());
        let account_id = matched_account
            .map(|a| a.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let created_at = matched_account
            .map(|a| a.created_at.clone())
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let expires_at = Utc::now() + chrono::Duration::seconds(mc_data.expires_in as i64);

        let mut refresh_token_val = ms_refresh_token.unwrap_or_default();

        let secret_key = format!("tokens:{}", account_id);
        let tokens = AccountTokens {
            access_token: mc_data.access_token.clone(),
            refresh_token: refresh_token_val.clone(),
        };
        let mut secret_payload = serde_json::to_string(&tokens).unwrap_or_default();
        let save_secret_res = self.secrets.set(&secret_key, &secret_payload).await;
        secret_payload.zeroize();
        save_secret_res.map_err(|e| {
            LauncherErrorPayload::auth_failed(format!(
                "Gagal menyimpan token ke SecretStore terenkripsi: {}",
                e.message
            ))
        })?;

        let active_skin = profile_data
            .skins
            .iter()
            .find(|s| s.state.as_deref() == Some("ACTIVE"))
            .or_else(|| profile_data.skins.first());
        let mut skin_url = active_skin.map(|s| s.url.clone());
        if let Some(ref mut url) = skin_url {
            if url.starts_with("http://") {
                *url = url.replace("http://", "https://");
            }
        }
        let skin_variant = active_skin.and_then(|s| s.variant.as_ref().map(|v| v.to_lowercase()));

        let active_cape = profile_data
            .capes
            .iter()
            .find(|c| c.state.as_deref() == Some("ACTIVE"));
        let active_cape_id = active_cape.and_then(|c| c.id.clone());
        let mut cape_url = active_cape.map(|c| c.url.clone());
        if let Some(ref mut url) = cape_url {
            if url.starts_with("http://") {
                *url = url.replace("http://", "https://");
            }
        }

        let owned_capes: Vec<launcher_types::AccountCape> = profile_data
            .capes
            .into_iter()
            .map(|c| {
                let url = if c.url.starts_with("http://") {
                    c.url.replace("http://", "https://")
                } else {
                    c.url
                };
                launcher_types::AccountCape {
                    id: c.id.unwrap_or_default(),
                    state: c.state.unwrap_or_else(|| "INACTIVE".to_string()),
                    url,
                    alias: c.alias,
                }
            })
            .collect();

        if launcher_security::validate_instance_id(&account_id).is_ok() {
            let skins_dir = self.paths.root.join("skins");
            if launcher_security::ensure_dir_within(&self.paths.root, &skins_dir, true).is_ok() {
                if let Some(ref url) = skin_url {
                    if let Ok(target_skin) =
                        launcher_security::safe_subpath(&skins_dir, &format!("{}.png", account_id))
                    {
                        if launcher_security::ensure_descendants_no_symlink(
                            &skins_dir,
                            &target_skin,
                        )
                        .is_ok()
                        {
                            if let Ok(res) = self.http.get(url).await {
                                if let Ok(bytes) = res.bytes().await {
                                    let _ = std::fs::write(&target_skin, bytes);
                                }
                            }
                        }
                    }
                }
                if let Ok(target_cape) =
                    launcher_security::safe_subpath(&skins_dir, &format!("{}_cape.png", account_id))
                {
                    if let Some(ref url) = cape_url {
                        if launcher_security::ensure_descendants_no_symlink(
                            &skins_dir,
                            &target_cape,
                        )
                        .is_ok()
                        {
                            if let Ok(res) = self.http.get(url).await {
                                if let Ok(bytes) = res.bytes().await {
                                    let _ = std::fs::write(&target_cape, bytes);
                                }
                            }
                        }
                    } else {
                        let _ =
                            launcher_security::safe_remove_file_within(&skins_dir, &target_cape);
                    }
                }
            }
        }

        refresh_token_val.zeroize();
        let account = Account {
            id: account_id,
            username: profile_data.name,
            uuid: formatted_uuid,
            account_type: AccountType::Microsoft,
            created_at,
            expires_at: Some(expires_at.to_rfc3339()),
            tokens: Some(tokens),
            xuid,
            skin_url,
            skin_variant,
            cape_url,
            active_cape_id,
            capes: owned_capes,
        };

        let mut account_for_disk = account.clone();
        account_for_disk.tokens = None;
        self.accounts.save_account(account_for_disk)?;

        Ok(account)
    }

    pub async fn sign_out(&self, account_id: &str) -> Result<(), LauncherErrorPayload> {
        let secret_key = format!("tokens:{}", account_id);
        let _ = self.secrets.delete(&secret_key).await;
        let _ = self.accounts.remove(account_id);
        Ok(())
    }

    async fn load_account_with_fresh_token(
        &self,
        account_id: &str,
    ) -> Result<(Account, String), LauncherErrorPayload> {
        let mut account = self
            .accounts
            .get(account_id)
            .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;

        let secret_key = format!("tokens:{}", account.id);
        let mut secret_data = self.secrets.get(&secret_key).await.ok_or_else(|| {
            LauncherErrorPayload::auth_failed(
                "Token autentikasi tidak ditemukan. Silakan login ulang.",
            )
        })?;

        let tokens = serde_json::from_str::<AccountTokens>(&secret_data);
        secret_data.zeroize();
        let tokens = tokens.map_err(|e| {
            LauncherErrorPayload::auth_failed(format!("Gagal membaca token: {}", e))
        })?;

        let needs_refresh = !Self::is_expiry_still_valid(account.expires_at.as_deref());

        let access_token = if needs_refresh {
            let refreshed = self.refresh_session(&account).await?;
            account = refreshed;
            account
                .tokens
                .as_ref()
                .map(|t| t.access_token.clone())
                .unwrap_or_else(|| tokens.access_token.clone())
        } else {
            tokens.access_token.clone()
        };

        Ok((account, access_token))
    }

    async fn refresh_bearer_for_retry(
        &self,
        account: &mut Account,
    ) -> Result<String, LauncherErrorPayload> {
        let refreshed = self.refresh_session(account).await?;
        let next_tok = refreshed
            .tokens
            .as_ref()
            .map(|t| t.access_token.clone())
            .ok_or_else(|| {
                LauncherErrorPayload::auth_failed(
                    "Token akses tidak tersedia setelah pembaruan sesi.",
                )
            })?;
        *account = refreshed;
        Ok(next_tok)
    }

    pub async fn sync_profile(&self, account_id: &str) -> Result<Account, LauncherErrorPayload> {
        let (mut account, mut access_token) =
            self.load_account_with_fresh_token(account_id).await?;

        let profile_url = "https://api.minecraftservices.com/minecraft/profile";
        let mut resp = self
            .http
            .request(Method::GET, profile_url)?
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| {
                LauncherErrorPayload::network(format!(
                    "Gagal menghubungi Minecraft profile API: {}",
                    e
                ))
            })?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            access_token = self.refresh_bearer_for_retry(&mut account).await?;
            resp = self
                .http
                .request(Method::GET, profile_url)?
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await
                .map_err(|e| {
                    LauncherErrorPayload::network(format!(
                        "Gagal menghubungi Minecraft profile API: {}",
                        e
                    ))
                })?;
        }

        if !resp.status().is_success() {
            return Err(LauncherErrorPayload::auth_failed(format!(
                "Minecraft profile API menolak request: status {}",
                resp.status()
            )));
        }

        let profile_data: MinecraftProfileResponse = resp.json().await.map_err(|e| {
            LauncherErrorPayload::auth_failed(format!("Format profil Minecraft tidak valid: {}", e))
        })?;

        let active_skin = profile_data
            .skins
            .iter()
            .find(|s| s.state.as_deref() == Some("ACTIVE"))
            .or_else(|| profile_data.skins.first());
        let mut skin_url = active_skin.map(|s| s.url.clone());
        if let Some(ref mut url) = skin_url {
            if url.starts_with("http://") {
                *url = url.replace("http://", "https://");
            }
        }
        let skin_variant = active_skin.and_then(|s| s.variant.as_ref().map(|v| v.to_lowercase()));

        let active_cape = profile_data
            .capes
            .iter()
            .find(|c| c.state.as_deref() == Some("ACTIVE"));
        let active_cape_id = active_cape.and_then(|c| c.id.clone());
        let mut cape_url = active_cape.map(|c| c.url.clone());
        if let Some(ref mut url) = cape_url {
            if url.starts_with("http://") {
                *url = url.replace("http://", "https://");
            }
        }

        let owned_capes: Vec<launcher_types::AccountCape> = profile_data
            .capes
            .into_iter()
            .map(|c| {
                let url = if c.url.starts_with("http://") {
                    c.url.replace("http://", "https://")
                } else {
                    c.url
                };
                launcher_types::AccountCape {
                    id: c.id.unwrap_or_default(),
                    state: c.state.unwrap_or_else(|| "INACTIVE".to_string()),
                    url,
                    alias: c.alias,
                }
            })
            .collect();

        if launcher_security::validate_instance_id(&account.id).is_ok() {
            let skins_dir = self.paths.root.join("skins");
            if launcher_security::ensure_dir_within(&self.paths.root, &skins_dir, true).is_ok() {
                if let Some(url_str) = skin_url.as_deref() {
                    if let Ok(target_skin) =
                        launcher_security::safe_subpath(&skins_dir, &format!("{}.png", account.id))
                    {
                        if launcher_security::ensure_descendants_no_symlink(
                            &skins_dir,
                            &target_skin,
                        )
                        .is_ok()
                        {
                            if let Ok(res) = self.http.get(url_str).await {
                                if let Ok(bytes) = res.bytes().await {
                                    let _ = std::fs::write(&target_skin, bytes);
                                }
                            }
                        }
                    }
                }

                if let Ok(target_cape) =
                    launcher_security::safe_subpath(&skins_dir, &format!("{}_cape.png", account.id))
                {
                    if let Some(url_str) = cape_url.as_deref() {
                        if launcher_security::ensure_descendants_no_symlink(
                            &skins_dir,
                            &target_cape,
                        )
                        .is_ok()
                        {
                            if let Ok(res) = self.http.get(url_str).await {
                                if let Ok(bytes) = res.bytes().await {
                                    let _ = std::fs::write(&target_cape, bytes);
                                }
                            }
                        }
                    } else {
                        let _ =
                            launcher_security::safe_remove_file_within(&skins_dir, &target_cape);
                    }
                }
            }
        }

        account.username = profile_data.name;
        account.skin_url = skin_url;
        account.skin_variant = skin_variant;
        account.cape_url = cape_url;
        account.active_cape_id = active_cape_id;
        account.capes = owned_capes;

        let mut account_for_disk = account.clone();
        account_for_disk.tokens = None;
        self.accounts.save_account(account_for_disk)?;

        Ok(account)
    }

    pub async fn set_active_cape(
        &self,
        account_id: &str,
        cape_id: Option<&str>,
    ) -> Result<Account, LauncherErrorPayload> {
        let (mut account, mut access_token) =
            self.load_account_with_fresh_token(account_id).await?;

        launcher_security::validate_instance_id(&account.id)?;
        let skins_dir = self.paths.root.join("skins");
        launcher_security::ensure_dir_within(&self.paths.root, &skins_dir, true)?;
        let target_cape =
            launcher_security::safe_subpath(&skins_dir, &format!("{}_cape.png", account.id))?;

        let api_url = "https://api.minecraftservices.com/minecraft/profile/capes/active";
        if let Some(cid) = cape_id {
            let req_body = serde_json::json!({ "capeId": cid });
            let mut resp = self
                .http
                .request(Method::PUT, api_url)?
                .header("Authorization", format!("Bearer {}", access_token))
                .json(&req_body)
                .send()
                .await
                .map_err(|e| {
                    LauncherErrorPayload::network(format!(
                        "Gagal menghubungi Mojang cape API: {}",
                        e
                    ))
                })?;

            if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                access_token = self.refresh_bearer_for_retry(&mut account).await?;
                resp = self
                    .http
                    .request(Method::PUT, api_url)?
                    .header("Authorization", format!("Bearer {}", access_token))
                    .json(&req_body)
                    .send()
                    .await
                    .map_err(|e| {
                        LauncherErrorPayload::network(format!(
                            "Gagal menghubungi Mojang cape API: {}",
                            e
                        ))
                    })?;
            }

            if !resp.status().is_success() {
                return Err(LauncherErrorPayload::auth_failed(format!(
                    "Gagal mengaktifkan jubah di server Mojang: status {}",
                    resp.status()
                )));
            }

            let mut matching_url: Option<String> = None;
            for c in &mut account.capes {
                if c.id == cid {
                    c.state = "ACTIVE".to_string();
                    matching_url = Some(c.url.clone());
                } else {
                    c.state = "INACTIVE".to_string();
                }
            }

            account.active_cape_id = Some(cid.to_string());
            account.cape_url = matching_url.clone();

            if let Some(ref url) = matching_url {
                if launcher_security::ensure_descendants_no_symlink(&skins_dir, &target_cape)
                    .is_ok()
                {
                    if let Ok(res) = self.http.get(url).await {
                        if let Ok(bytes) = res.bytes().await {
                            let _ = std::fs::write(&target_cape, bytes);
                        }
                    }
                }
            }
        } else {
            let mut resp = self
                .http
                .request(Method::DELETE, api_url)?
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await
                .map_err(|e| {
                    LauncherErrorPayload::network(format!(
                        "Gagal menghubungi Mojang cape API: {}",
                        e
                    ))
                })?;

            if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                access_token = self.refresh_bearer_for_retry(&mut account).await?;
                resp = self
                    .http
                    .request(Method::DELETE, api_url)?
                    .header("Authorization", format!("Bearer {}", access_token))
                    .send()
                    .await
                    .map_err(|e| {
                        LauncherErrorPayload::network(format!(
                            "Gagal menghubungi Mojang cape API: {}",
                            e
                        ))
                    })?;
            }

            if !resp.status().is_success() {
                return Err(LauncherErrorPayload::auth_failed(format!(
                    "Gagal menonaktifkan jubah di server Mojang: status {}",
                    resp.status()
                )));
            }

            for c in &mut account.capes {
                c.state = "INACTIVE".to_string();
            }
            account.active_cape_id = None;
            account.cape_url = None;
            let _ = launcher_security::safe_remove_file_within(&skins_dir, &target_cape);
        }

        let mut account_for_disk = account.clone();
        account_for_disk.tokens = None;
        self.accounts.save_account(account_for_disk)?;

        Ok(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[tokio::test]
    async fn test_n15_exchange_and_save_propagates_save_account_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://127.0.0.1:{}", addr.port());

        let handle = std::thread::spawn(move || {
            for _ in 0..5 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf);
                    let req = String::from_utf8_lossy(&buf);
                    let body = if req.contains("POST /xbox") || req.contains("POST /xsts") {
                        r#"{"Token":"tkn","DisplayClaims":{"xui":[{"uhs":"u1","xid":"x1"}]}}"#
                    } else if req.contains("POST /mc-login") {
                        r#"{"access_token":"mc-access","expires_in":3600}"#
                    } else if req.contains("GET /mc-profile") {
                        r#"{"id":"5627dd98e6be3c21b8a8e92344183641","name":"Steve","skins":[],"capes":[]}"#
                    } else {
                        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
                        break;
                    };
                    let hdr = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(hdr.as_bytes());
                    let _ = stream.write_all(body.as_bytes());
                }
            }
        });

        let tmp = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(tmp.path());
        let accounts = AccountsStore::new(paths.clone());
        let secrets = SecretStore::new(paths.secrets_file.clone());
        let _ = std::fs::remove_file(&paths.accounts_file);
        std::fs::create_dir_all(&paths.accounts_file).unwrap();

        let service = MicrosoftAuthService::new_loopback_test(paths, accounts, secrets);
        let err = service
            .exchange_and_save_with_urls(
                "ms-access".to_string(),
                Some("ms-refresh".to_string()),
                None,
                (
                    &format!("{}/xbox", base),
                    &format!("{}/xsts", base),
                    &format!("{}/mc-login", base),
                    &format!("{}/mc-profile", base),
                ),
            )
            .await
            .expect_err(
                "exchange_and_save must propagate save_account failure instead of returning Ok",
            );
        assert_eq!(err.code, launcher_types::ErrorCode::Internal);

        let _ = HttpClient::new_loopback_test()
            .get(&format!("{}/__shutdown", base))
            .await;
        let _ = handle.join();
    }

    #[tokio::test]
    async fn test_h10_relogin_deduplicates_uuid_and_clears_inactive_cape() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://127.0.0.1:{}", addr.port());

        let _handle = std::thread::spawn(move || {
            for _ in 0..8 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf);
                    let req = String::from_utf8_lossy(&buf);
                    let body = if req.contains("POST /xbox") || req.contains("POST /xsts") {
                        r#"{"Token":"tkn","DisplayClaims":{"xui":[{"uhs":"u1","xid":"x1"}]}}"#
                    } else if req.contains("POST /mc-login") {
                        r#"{"access_token":"mc-access","expires_in":3600}"#
                    } else if req.contains("GET /mc-profile") {
                        r#"{"id":"5627dd98e6be3c21b8a8e92344183641","name":"Steve","skins":[],"capes":[{"id":"c1","state":"INACTIVE","url":"https://textures.minecraft.net/cape.png"}]}"#
                    } else {
                        break;
                    };
                    let hdr = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(hdr.as_bytes());
                    let _ = stream.write_all(body.as_bytes());
                }
            }
        });

        let tmp = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(tmp.path());
        let accounts = AccountsStore::new(paths.clone());
        let secrets = SecretStore::new(paths.secrets_file.clone());
        let service =
            MicrosoftAuthService::new_loopback_test(paths.clone(), accounts.clone(), secrets);

        let first = service
            .exchange_and_save_with_urls(
                "ms-access".to_string(),
                Some("ms-refresh".to_string()),
                None,
                (
                    &format!("{}/xbox", base),
                    &format!("{}/xsts", base),
                    &format!("{}/mc-login", base),
                    &format!("{}/mc-profile", base),
                ),
            )
            .await
            .unwrap();
        assert!(first.active_cape_id.is_none());
        assert!(first.cape_url.is_none());

        let stale_cape = paths
            .root
            .join("skins")
            .join(format!("{}_cape.png", first.id));
        std::fs::create_dir_all(stale_cape.parent().unwrap()).unwrap();
        std::fs::write(&stale_cape, b"stale").unwrap();

        let second = service
            .exchange_and_save_with_urls(
                "ms-access-2".to_string(),
                Some("ms-refresh-2".to_string()),
                None,
                (
                    &format!("{}/xbox", base),
                    &format!("{}/xsts", base),
                    &format!("{}/mc-login", base),
                    &format!("{}/mc-profile", base),
                ),
            )
            .await
            .unwrap();
        assert_eq!(second.id, first.id);
        assert_eq!(accounts.list().len(), 1);
        assert!(!stale_cape.exists());
    }

    #[tokio::test]
    async fn test_h10_expiry_missing_or_corrupt_triggers_refresh_and_cancel_aborts_login() {
        assert!(!MicrosoftAuthService::is_expiry_still_valid(None));
        assert!(!MicrosoftAuthService::is_expiry_still_valid(Some(
            "not-a-date"
        )));

        let tmp = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(tmp.path());
        let accounts = AccountsStore::new(paths.clone());
        let secrets = SecretStore::new(paths.secrets_file.clone());
        let service = MicrosoftAuthService::new_loopback_test(paths, accounts, secrets);
        let cancel = Arc::new(AtomicBool::new(true));
        let err = service
            .complete_login_with_cancel(
                DeviceCodeInfo {
                    device_code: "dev".to_string(),
                    user_code: "USER".to_string(),
                    verification_uri: "https://microsoft.com/link".to_string(),
                    expires_in: u64::MAX,
                    interval: 5,
                },
                Some(cancel),
            )
            .await
            .expect_err("Cancelled login must return Cancelled immediately");
        assert_eq!(err.code, launcher_types::ErrorCode::Cancelled);
    }
}
