use chrono::Utc;
use launcher_net::HttpClient;
use launcher_platform::{DataPaths, SecretStore};
use launcher_storage::AccountsStore;
use launcher_types::{Account, AccountTokens, AccountType, DeviceCodeInfo, LauncherErrorPayload};
use serde::Deserialize;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;

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
}

impl MicrosoftAuthService {
    pub fn new(paths: DataPaths, accounts: AccountsStore, secrets: SecretStore) -> Self {
        Self {
            paths,
            http: HttpClient::new(),
            accounts,
            secrets,
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
            .client()
            .post(DEVICE_CODE_URL)
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
                "Microsoft menolak permintaan kode login (HTTP {}): {}",
                status, text
            )));
        }

        let data: DeviceCodeResponse = serde_json::from_str(&text).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!(
                "Microsoft tidak memberikan kode login yang valid: {} (isi: {})",
                e, text
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
        let client_id = self.client_id().ok_or_else(|| {
            LauncherErrorPayload::provider_not_configured("Client ID Microsoft tidak ditemukan.")
        })?;

        let deadline = std::time::Instant::now() + Duration::from_secs(code.expires_in);
        let mut interval_secs = code.interval.max(1);

        let token_resp = loop {
            if std::time::Instant::now() >= deadline {
                return Err(LauncherErrorPayload::auth_failed(
                    "Kode login kedaluwarsa. Mulai ulang proses masuk.",
                ));
            }

            let res = self
                .http
                .client()
                .post(TOKEN_URL)
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

            let text = res.text().await.map_err(|e| {
                LauncherErrorPayload::network(format!(
                    "Gagal membaca respons token server Microsoft: {}",
                    e
                ))
            })?;

            let body: TokenResponse = serde_json::from_str(&text).map_err(|e| {
                LauncherErrorPayload::auth_failed(format!(
                    "Format respons token tidak valid: {} (isi: {})",
                    e, text
                ))
            })?;

            if let Some(ref err) = body.error {
                match err.as_str() {
                    "authorization_pending" => {
                        sleep(Duration::from_secs(interval_secs)).await;
                        continue;
                    }
                    "slow_down" => {
                        interval_secs += 5;
                        sleep(Duration::from_secs(interval_secs)).await;
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

        self.exchange_and_save(token_resp.0, token_resp.1, None)
            .await
    }

    pub async fn refresh_session(
        &self,
        account: &Account,
    ) -> Result<Account, LauncherErrorPayload> {
        let client_id = self.client_id().ok_or_else(|| {
            LauncherErrorPayload::provider_not_configured("Client ID Microsoft tidak ditemukan.")
        })?;

        let secret_key = format!("tokens:{}", account.id);
        let stored_secret = self.secrets.get(&secret_key).await;
        let refresh_token = stored_secret
            .as_deref()
            .and_then(|data| {
                serde_json::from_str::<AccountTokens>(data)
                    .map(|t| t.refresh_token)
                    .ok()
                    .or_else(|| Some(data.to_string()))
            })
            .or_else(|| account.tokens.as_ref().map(|t| t.refresh_token.clone()))
            .ok_or_else(|| {
                LauncherErrorPayload::auth_failed(
                    "Sesi akun Microsoft tidak ditemukan. Silakan masuk kembali.",
                )
            })?;

        let res = self
            .http
            .client()
            .post(TOKEN_URL)
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

        let text = res.text().await.map_err(|e| {
            LauncherErrorPayload::network(format!("Gagal membaca respons server Microsoft: {}", e))
        })?;

        let token_resp: TokenResponse = serde_json::from_str(&text).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!(
                "Format respons refresh token tidak valid: {} (isi: {})",
                e, text
            ))
        })?;

        if let Some(err) = token_resp.error {
            return Err(LauncherErrorPayload::auth_failed(
                token_resp.error_description.unwrap_or(err),
            ));
        }

        let access_token = token_resp.access_token.ok_or_else(|| {
            LauncherErrorPayload::auth_failed(
                token_resp
                    .error_description
                    .unwrap_or_else(|| "Gagal memperbarui token Microsoft.".to_string()),
            )
        })?;

        self.exchange_and_save(
            access_token,
            token_resp.refresh_token.or(Some(refresh_token)),
            Some(account),
        )
        .await
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
            .client()
            .post(xbox_user_url)
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
        let xbox_text = xbox_res.text().await.unwrap_or_default();
        if !xbox_status.is_success() {
            return Err(LauncherErrorPayload::auth_failed(format!(
                "Xbox Live menolak token Microsoft: {}",
                xbox_text
            )));
        }

        let xbox_data: XboxUserResponse = serde_json::from_str(&xbox_text).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!(
                "Format respons Xbox Live tidak valid: {} (isi: {})",
                e, xbox_text
            ))
        })?;

        let first_claim =
            xbox_data.display_claims.xui.first().ok_or_else(|| {
                LauncherErrorPayload::auth_failed("Klaim Xbox Live tidak ditemukan.")
            })?;
        let uhs = first_claim.uhs.clone();
        let xuid = first_claim.xid.clone();

        let xsts_res = self
            .http
            .client()
            .post(xsts_url)
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

        let xsts_status = xsts_res.status();
        let xsts_text = xsts_res.text().await.unwrap_or_default();
        if !xsts_status.is_success() {
            if xsts_text.contains("2148916238") {
                return Err(LauncherErrorPayload::auth_failed(
                    "Akun Microsoft ini merupakan akun anak di bawah umur yang memerlukan persetujuan orang tua (Xbox Family).",
                ));
            } else if xsts_text.contains("2148916233") {
                return Err(LauncherErrorPayload::auth_failed(
                    "Akun ini belum memiliki profil Xbox Live. Silakan buka xbox.com dan buat gamertag terlebih dahulu.",
                ));
            } else if xsts_text.contains("2148916235") {
                return Err(LauncherErrorPayload::auth_failed(
                    "Xbox Live belum tersedia di negara/wilayah akun Anda.",
                ));
            } else if xsts_text.contains("2148916236") || xsts_text.contains("2148916237") {
                return Err(LauncherErrorPayload::auth_failed(
                    "Akun ini memerlukan verifikasi usia dewasa (persyaratan regulasi setempat).",
                ));
            } else {
                return Err(LauncherErrorPayload::auth_failed(format!(
                    "XSTS menolak token Xbox Live: {}",
                    xsts_text
                )));
            }
        }

        let xsts_data: XstsResponse = serde_json::from_str(&xsts_text).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!(
                "Format respons XSTS tidak valid: {} (isi: {})",
                e, xsts_text
            ))
        })?;

        let mc_res = self
            .http
            .client()
            .post(mc_login_url)
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

        let mc_status = mc_res.status();
        let mc_text = mc_res.text().await.unwrap_or_default();
        if !mc_status.is_success() {
            return Err(LauncherErrorPayload::auth_failed(format!(
                "Minecraft Services menolak login: {}",
                mc_text
            )));
        }

        let mc_data: MinecraftLoginResponse = serde_json::from_str(&mc_text).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!(
                "Format respons Minecraft Services tidak valid: {} (isi: {})",
                e, mc_text
            ))
        })?;

        let profile_res = self
            .http
            .client()
            .get(mc_profile_url)
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
                "Gagal mengambil profil Minecraft: {}",
                profile_text
            )));
        }

        let profile_data: MinecraftProfileResponse = serde_json::from_str(&profile_text).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!(
                "Akun Minecraft tidak ditemukan atau belum memiliki lisensi Java Edition: {} (isi: {})",
                e, profile_text
            ))
        })?;

        let formatted_uuid = format_uuid(&profile_data.id)?;
        let account_id = existing_account
            .map(|a| a.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let created_at = existing_account
            .map(|a| a.created_at.clone())
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let expires_at = Utc::now() + chrono::Duration::seconds(mc_data.expires_in as i64);

        let refresh_token_val = ms_refresh_token.unwrap_or_default();

        let secret_key = format!("tokens:{}", account_id);
        let secret_payload = serde_json::to_string(&AccountTokens {
            access_token: mc_data.access_token.clone(),
            refresh_token: refresh_token_val.clone(),
        })
        .unwrap_or_default();
        self.secrets
            .set(&secret_key, &secret_payload)
            .await
            .map_err(|e| {
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
            .find(|c| c.state.as_deref() == Some("ACTIVE"))
            .or_else(|| profile_data.capes.first());
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
                        if let Ok(res) = self.http.get(url).await {
                            if let Ok(bytes) = res.bytes().await {
                                let _ = std::fs::write(&target_skin, bytes);
                            }
                        }
                    }
                }
                if let Some(ref url) = cape_url {
                    if let Ok(target_cape) = launcher_security::safe_subpath(
                        &skins_dir,
                        &format!("{}_cape.png", account_id),
                    ) {
                        if let Ok(res) = self.http.get(url).await {
                            if let Ok(bytes) = res.bytes().await {
                                let _ = std::fs::write(&target_cape, bytes);
                            }
                        }
                    }
                }
            }
        }

        let account = Account {
            id: account_id,
            username: profile_data.name,
            uuid: formatted_uuid,
            account_type: AccountType::Microsoft,
            created_at,
            expires_at: Some(expires_at.to_rfc3339()),
            tokens: Some(AccountTokens {
                access_token: mc_data.access_token,
                refresh_token: refresh_token_val,
            }),
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

    pub async fn sync_profile(&self, account_id: &str) -> Result<Account, LauncherErrorPayload> {
        let mut account = self
            .accounts
            .get(account_id)
            .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;

        let secret_key = format!("tokens:{}", account.id);
        let secret_data = self.secrets.get(&secret_key).await.ok_or_else(|| {
            LauncherErrorPayload::auth_failed(
                "Token autentikasi tidak ditemukan. Silakan login ulang.",
            )
        })?;

        let tokens = serde_json::from_str::<AccountTokens>(&secret_data).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!("Gagal membaca token: {}", e))
        })?;

        let needs_refresh = if let Some(ref exp_str) = account.expires_at {
            if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(exp_str) {
                exp.with_timezone(&chrono::Utc) <= chrono::Utc::now() + chrono::Duration::minutes(5)
            } else {
                false
            }
        } else {
            false
        };

        let access_token = if needs_refresh {
            let refreshed = self.refresh_session(&account).await?;
            account = refreshed;
            account
                .tokens
                .as_ref()
                .map(|t| t.access_token.clone())
                .unwrap_or(tokens.access_token)
        } else {
            tokens.access_token
        };

        let profile_url = "https://api.minecraftservices.com/minecraft/profile";
        let resp = self
            .http
            .client()
            .get(profile_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| {
                LauncherErrorPayload::network(format!(
                    "Gagal menghubungi Minecraft profile API: {}",
                    e
                ))
            })?;

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
                        if let Ok(res) = self.http.get(url_str).await {
                            if let Ok(bytes) = res.bytes().await {
                                let _ = std::fs::write(&target_skin, bytes);
                            }
                        }
                    }
                }

                if let Ok(target_cape) =
                    launcher_security::safe_subpath(&skins_dir, &format!("{}_cape.png", account.id))
                {
                    if let Some(url_str) = cape_url.as_deref() {
                        if let Ok(res) = self.http.get(url_str).await {
                            if let Ok(bytes) = res.bytes().await {
                                let _ = std::fs::write(&target_cape, bytes);
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
        let mut account = self
            .accounts
            .get(account_id)
            .ok_or_else(|| LauncherErrorPayload::not_found("Akun tidak ditemukan."))?;

        let secret_key = format!("tokens:{}", account.id);
        let secret_data = self.secrets.get(&secret_key).await.ok_or_else(|| {
            LauncherErrorPayload::auth_failed(
                "Token autentikasi tidak ditemukan. Silakan login ulang.",
            )
        })?;

        let tokens = serde_json::from_str::<AccountTokens>(&secret_data).map_err(|e| {
            LauncherErrorPayload::auth_failed(format!("Gagal membaca token: {}", e))
        })?;

        launcher_security::validate_instance_id(&account.id)?;
        let skins_dir = self.paths.root.join("skins");
        launcher_security::ensure_dir_within(&self.paths.root, &skins_dir, true)?;
        let target_cape =
            launcher_security::safe_subpath(&skins_dir, &format!("{}_cape.png", account.id))?;

        if let Some(cid) = cape_id {
            let api_url = "https://api.minecraftservices.com/minecraft/profile/capes/active";
            let req_body = serde_json::json!({ "capeId": cid });
            let resp = self
                .http
                .client()
                .put(api_url)
                .header("Authorization", format!("Bearer {}", tokens.access_token))
                .json(&req_body)
                .send()
                .await
                .map_err(|e| {
                    LauncherErrorPayload::network(format!(
                        "Gagal menghubungi Mojang cape API: {}",
                        e
                    ))
                })?;

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
                if let Ok(res) = self.http.get(url).await {
                    if let Ok(bytes) = res.bytes().await {
                        let _ = std::fs::write(&target_cape, bytes);
                    }
                }
            }
        } else {
            let api_url = "https://api.minecraftservices.com/minecraft/profile/capes/active";
            let _ = self
                .http
                .client()
                .delete(api_url)
                .header("Authorization", format!("Bearer {}", tokens.access_token))
                .send()
                .await;

            for c in &mut account.capes {
                c.state = "INACTIVE".to_string();
            }
            account.active_cape_id = None;
            account.cape_url = None;
            let _ = std::fs::remove_file(&target_cape);
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

        let service = MicrosoftAuthService::new(paths, accounts, secrets);
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

        let _ = HttpClient::new().get(&format!("{}/__shutdown", base)).await;
        let _ = handle.join();
    }
}
