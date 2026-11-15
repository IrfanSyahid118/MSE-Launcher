use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use launcher_types::DiscordSettings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

pub const DEFAULT_DISCORD_CLIENT_ID: &str = "1570736748328132668";
pub const LAUNCHER_RPC_NAME: &str = "MSE Launcher";
pub const LAUNCHER_RPC_IDLE_DESCRIPTION: &str = "Exploring....";
// Replace with your newly uploaded HTTPS URL or Discord Portal asset key once uploaded.
pub const LAUNCHER_RPC_ICON_URL: &str =
    "https://cdn.discordapp.com/app-assets/1570736748328132668/1571144659827167333.png";

#[derive(Debug, Clone)]
pub enum ActivityState {
    Idle,
    Playing {
        instance_name: String,
        version: String,
        loader: String,
        icon_url: Option<String>,
        started_at: i64,
    },
    Clear,
}

#[derive(Debug)]
enum RpcCommand {
    SetActivity(ActivityState),
    UpdateSettings(DiscordSettings),
    Shutdown,
}

#[derive(Clone)]
pub struct DiscordRpcService {
    tx: Sender<RpcCommand>,
    running: Arc<AtomicBool>,
    // Track only service handles (not the worker thread) so Drop shuts down cleanly.
    handles: Arc<()>,
    app_start_epoch: i64,
    idle_updates: Arc<std::sync::atomic::AtomicUsize>,
}

pub fn should_dispatch_activity(
    client_connected: bool,
    just_connected: bool,
    activity_dirty: bool,
) -> bool {
    client_connected && (just_connected || activity_dirty)
}

fn resolve_client_id(settings: &DiscordSettings) -> String {
    settings
        .custom_client_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_DISCORD_CLIENT_ID)
        .to_string()
}

fn apply_settings_update(
    new_settings: DiscordSettings,
    current_client_id: &mut String,
    settings: &mut DiscordSettings,
    client: &mut Option<DiscordIpcClient>,
) {
    let new_client_id = resolve_client_id(&new_settings);
    let client_id_changed = new_client_id != *current_client_id;
    let enabled_changed = new_settings.enabled != settings.enabled;

    *settings = new_settings;
    *current_client_id = new_client_id;

    if client_id_changed || (enabled_changed && !settings.enabled) {
        if let Some(mut c) = client.take() {
            let _ = c.clear_activity();
            let _ = c.close();
        }
    }
}

fn build_idle_activity<'a>(app_start_epoch: i64) -> Activity<'a> {
    Activity::new()
        .details(LAUNCHER_RPC_NAME)
        .state(LAUNCHER_RPC_IDLE_DESCRIPTION)
        .timestamps(Timestamps::new().start(app_start_epoch))
        .assets(
            Assets::new()
                .large_image(LAUNCHER_RPC_ICON_URL)
                .large_text(LAUNCHER_RPC_NAME),
        )
}

fn resolve_playing_image_url(icon_url: Option<&str>) -> Option<&str> {
    let trimmed = icon_url?.trim();
    // Discord Rich Presence asset fields only accept HTTPS URLs up to 256 chars (not data: URIs).
    if trimmed.starts_with("https://")
        && trimmed.len() <= 256
        && !trimmed.chars().any(char::is_whitespace)
    {
        Some(trimmed)
    } else {
        None
    }
}

fn build_playing_activity<'a>(
    instance_name: &'a str,
    state_str: &'a str,
    details_str: &'a str,
    icon_url: Option<&'a str>,
    started_at: i64,
) -> Activity<'a> {
    let assets = if let Some(modpack_icon) = resolve_playing_image_url(icon_url) {
        Assets::new()
            .large_image(modpack_icon)
            .large_text(instance_name)
            .small_image(LAUNCHER_RPC_ICON_URL)
            .small_text(LAUNCHER_RPC_NAME)
    } else {
        Assets::new()
            .large_image(LAUNCHER_RPC_ICON_URL)
            .large_text(instance_name)
    };

    Activity::new()
        .state(state_str)
        .details(details_str)
        .timestamps(Timestamps::new().start(started_at))
        .assets(assets)
}

impl DiscordRpcService {
    pub fn new(initial_settings: DiscordSettings) -> Self {
        let app_start_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let (tx, rx) = channel::<RpcCommand>();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        // Dedicated OS thread so Discord named pipe FFI does not block Tokio
        std::thread::Builder::new()
            .name("discord-rpc-worker".into())
            .spawn(move || {
                Self::worker_loop(rx, running_clone, initial_settings, app_start_epoch);
            })
            .expect("Failed to spawn discord-rpc-worker thread");

        Self {
            tx,
            running,
            handles: Arc::new(()),
            app_start_epoch,
            idle_updates: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    fn worker_loop(
        rx: Receiver<RpcCommand>,
        running: Arc<AtomicBool>,
        initial_settings: DiscordSettings,
        app_start_epoch: i64,
    ) {
        let mut settings = initial_settings;
        let mut current_client_id = resolve_client_id(&settings);

        let mut client: Option<DiscordIpcClient> = None;
        let mut target_state = if settings.enabled && settings.show_idle {
            ActivityState::Idle
        } else {
            ActivityState::Clear
        };
        let mut activity_dirty = true;

        let mut last_connect_attempt = Instant::now()
            .checked_sub(Duration::from_secs(30))
            .unwrap_or_else(Instant::now);

        while running.load(Ordering::Relaxed) {
            let recv_res = rx.recv_timeout(Duration::from_secs(3));
            let mut state_changed = false;

            match recv_res {
                Ok(RpcCommand::Shutdown) => break,
                Ok(RpcCommand::SetActivity(new_state)) => {
                    target_state = new_state;
                    state_changed = true;
                    activity_dirty = true;
                }
                Ok(RpcCommand::UpdateSettings(new_settings)) => {
                    apply_settings_update(
                        new_settings,
                        &mut current_client_id,
                        &mut settings,
                        &mut client,
                    );
                    state_changed = true;
                    activity_dirty = true;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    RpcCommand::Shutdown => {
                        running.store(false, Ordering::Relaxed);
                        break;
                    }
                    RpcCommand::SetActivity(new_state) => {
                        target_state = new_state;
                        state_changed = true;
                        activity_dirty = true;
                    }
                    RpcCommand::UpdateSettings(new_settings) => {
                        apply_settings_update(
                            new_settings,
                            &mut current_client_id,
                            &mut settings,
                            &mut client,
                        );
                        state_changed = true;
                        activity_dirty = true;
                    }
                }
            }

            if !running.load(Ordering::Relaxed) {
                break;
            }

            if !settings.enabled {
                if let Some(mut c) = client.take() {
                    let _ = c.clear_activity();
                    let _ = c.close();
                    activity_dirty = true;
                }
                continue;
            }

            let mut just_connected = false;
            if client.is_none() {
                let now = Instant::now();
                if state_changed || now.duration_since(last_connect_attempt).as_secs() >= 5 {
                    last_connect_attempt = now;
                    let mut new_c = DiscordIpcClient::new(&current_client_id);
                    if let Ok(()) = new_c.connect() {
                        info!(
                            "Connected to Discord IPC (client_id: {})",
                            current_client_id
                        );
                        client = Some(new_c);
                        just_connected = true;
                    } else {
                        debug!("Discord IPC socket not available");
                    }
                }
            }

            if should_dispatch_activity(client.is_some(), just_connected, activity_dirty) {
                if let Some(ref mut c) = client {
                    let apply_res = match &target_state {
                        ActivityState::Idle => {
                            if settings.show_idle {
                                c.set_activity(build_idle_activity(app_start_epoch))
                            } else {
                                c.clear_activity()
                            }
                        }
                        ActivityState::Playing {
                            instance_name,
                            version,
                            loader,
                            icon_url,
                            started_at,
                        } => {
                            if settings.show_playing {
                                let details_str = format!("{loader} {version}");
                                let state_str = format!("Playing {instance_name}");
                                let act = build_playing_activity(
                                    instance_name,
                                    &state_str,
                                    &details_str,
                                    icon_url.as_deref(),
                                    *started_at,
                                );
                                c.set_activity(act)
                            } else if settings.show_idle {
                                c.set_activity(build_idle_activity(app_start_epoch))
                            } else {
                                c.clear_activity()
                            }
                        }
                        ActivityState::Clear => c.clear_activity(),
                    };

                    if let Err(e) = apply_res {
                        warn!("Discord IPC update failed: {:?}", e);
                        let _ = c.close();
                        client = None;
                        activity_dirty = true;
                    } else {
                        activity_dirty = false;
                    }
                }
            }
        }

        if let Some(mut c) = client.take() {
            let _ = c.clear_activity();
            let _ = c.close();
        }
        running.store(false, Ordering::Relaxed);
    }

    pub fn set_idle(&self) {
        self.idle_updates.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.send(RpcCommand::SetActivity(ActivityState::Idle));
    }

    pub fn idle_update_count(&self) -> usize {
        self.idle_updates.load(Ordering::Relaxed)
    }

    pub fn set_playing(
        &self,
        instance_name: String,
        version: String,
        loader: String,
        icon_url: Option<String>,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(self.app_start_epoch);

        let _ = self
            .tx
            .send(RpcCommand::SetActivity(ActivityState::Playing {
                instance_name,
                version,
                loader,
                icon_url,
                started_at: now,
            }));
    }

    pub fn clear_activity(&self) {
        let _ = self.tx.send(RpcCommand::SetActivity(ActivityState::Clear));
    }

    pub fn update_settings(&self, settings: DiscordSettings) {
        let _ = self.tx.send(RpcCommand::UpdateSettings(settings));
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
        let _ = self.tx.send(RpcCommand::Shutdown);
    }
}

impl Drop for DiscordRpcService {
    fn drop(&mut self) {
        if Arc::strong_count(&self.handles) == 1 {
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_service_lifecycle() {
        let settings = DiscordSettings {
            enabled: false,
            show_idle: true,
            show_playing: true,
            custom_client_id: None,
        };

        let service = DiscordRpcService::new(settings.clone());
        service.set_idle();
        service.set_playing(
            "Survival World".to_string(),
            "1.21.4".to_string(),
            "Fabric".to_string(),
            Some("https://cdn.modrinth.com/data/1KVo5zza/icon.webp".to_string()),
        );
        service.update_settings(DiscordSettings {
            enabled: true,
            ..settings
        });
        service.clear_activity();
        service.shutdown();
    }

    #[test]
    fn test_n31_drop_last_service_stops_worker_thread() {
        let settings = DiscordSettings {
            enabled: false,
            show_idle: true,
            show_playing: true,
            custom_client_id: None,
        };

        let service = DiscordRpcService::new(settings);
        let running = Arc::clone(&service.running);
        let cloned = service.clone();

        drop(cloned);
        std::thread::sleep(Duration::from_millis(20));
        assert!(
            running.load(Ordering::Relaxed),
            "Worker must stay alive while another DiscordRpcService clone exists"
        );

        drop(service);
        for _ in 0..30 {
            if !running.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !running.load(Ordering::Relaxed),
            "Dropping last DiscordRpcService handle must call shutdown() and stop worker"
        );
    }

    #[test]
    fn test_n31_should_dispatch_activity_skips_redundant_poll_ticks() {
        assert!(!should_dispatch_activity(false, false, false));
        assert!(!should_dispatch_activity(false, false, true));
        assert!(!should_dispatch_activity(true, false, false));
        assert!(should_dispatch_activity(true, true, false));
        assert!(should_dispatch_activity(true, false, true));
        assert!(should_dispatch_activity(true, true, true));

        let serialized = serde_json::to_value(build_idle_activity(123)).unwrap();
        assert_eq!(serialized["details"], LAUNCHER_RPC_NAME);
        assert_eq!(serialized["state"], "Exploring....");
        assert_eq!(serialized["assets"]["large_text"], LAUNCHER_RPC_NAME);
        assert_eq!(serialized["assets"]["large_image"], LAUNCHER_RPC_ICON_URL);
    }

    #[test]
    fn test_build_playing_activity_uses_https_modpack_icon_only() {
        let modpack_url = "https://cdn.modrinth.com/data/1KVo5zza/icon.webp";
        let with_pack = serde_json::to_value(build_playing_activity(
            "Fabulously Optimized",
            "Playing Fabulously Optimized",
            "Fabric 1.21.4",
            Some(modpack_url),
            456,
        ))
        .unwrap();
        assert_eq!(with_pack["assets"]["large_image"], modpack_url);
        assert_eq!(with_pack["assets"]["large_text"], "Fabulously Optimized");
        assert_eq!(with_pack["assets"]["small_image"], LAUNCHER_RPC_ICON_URL);
        assert_eq!(with_pack["assets"]["small_text"], LAUNCHER_RPC_NAME);

        let with_data_uri = serde_json::to_value(build_playing_activity(
            "Custom Pack",
            "Playing Custom Pack",
            "Fabric 1.21.4",
            Some("data:image/png;base64,iVBORw0KGgo="),
            456,
        ))
        .unwrap();
        assert_eq!(
            with_data_uri["assets"]["large_image"],
            LAUNCHER_RPC_ICON_URL
        );
        assert!(with_data_uri["assets"]["small_image"].is_null());
    }
}
