use crate::state::AppState;
use launcher_types::LauncherErrorPayload;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;
use uuid::Uuid;

fn active_tasks() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static TASKS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn register_task(task_id: &str) -> Arc<AtomicBool> {
    let cancelled = Arc::new(AtomicBool::new(false));
    if let Ok(mut tasks) = active_tasks().lock() {
        tasks.insert(task_id.to_string(), Arc::clone(&cancelled));
    }
    cancelled
}

pub(crate) struct ActiveTaskGuard {
    task_id: String,
}

impl ActiveTaskGuard {
    pub fn new(task_id: String) -> Self {
        Self { task_id }
    }
}

impl Drop for ActiveTaskGuard {
    fn drop(&mut self) {
        if let Ok(mut tasks) = active_tasks().lock() {
            tasks.remove(&self.task_id);
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchStartRequest {
    pub instance_id: String,
    pub version_id: String,
    pub server_address: Option<String>,
}

#[tauri::command]
pub async fn launch_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    instance_id: String,
    version_id: String,
    server_address: Option<String>,
) -> Result<Value, LauncherErrorPayload> {
    let task_id = Uuid::new_v4().to_string();
    let cancelled = register_task(&task_id);

    let app_clone = app.clone();
    let task_id_clone = task_id.clone();
    let inst_id_clone = instance_id.clone();

    let launch_service = state.launch.clone();
    let discord = state.discord.clone();
    let logs = state.logs.clone();
    let instance_info = state.instances.get(&instance_id).ok().map(|inst| {
        (
            inst.name.clone(),
            inst.minecraft_version.clone(),
            format!("{:?}", inst.loader.loader_type),
            inst.icon_url.clone(),
        )
    });

    if let Some((ref name, ref mc_ver, ref loader, _)) = instance_info {
        logs.record_entry(
            "system",
            format!(
                "Memulai peluncuran instance '{}' (Minecraft {}, loader {})",
                name, mc_ver, loader
            ),
        );
    } else {
        logs.record_entry(
            "system",
            format!(
                "Memulai peluncuran instance '{}' (versi {})",
                instance_id, version_id
            ),
        );
    }

    tokio::spawn(async move {
        let _guard = ActiveTaskGuard::new(task_id_clone.clone());
        let app1 = app_clone.clone();
        let inst_id1 = inst_id_clone.clone();
        let on_log = move |lines: Vec<launcher_launch::LogLine>| {
            let _ = app1.emit(
                "game:log",
                json!({
                    "instanceId": inst_id1,
                    "lines": lines,
                }),
            );
        };

        let app2 = app_clone.clone();
        let inst_id2 = inst_id_clone.clone();
        let discord_exit = discord.clone();
        let logs_exit = logs.clone();
        let on_exit = move |code: Option<i32>| {
            discord_exit.set_idle();
            match code {
                Some(0) => logs_exit.record_entry(
                    "system",
                    format!("Proses game '{}' selesai normal (exit code 0).", inst_id2),
                ),
                Some(c) => logs_exit.record_entry(
                    "stderr",
                    format!(
                        "Proses game '{}' berhenti dengan exit code {}.",
                        inst_id2, c
                    ),
                ),
                None => logs_exit.record_entry(
                    "stderr",
                    format!("Proses game '{}' berhenti tanpa exit code.", inst_id2),
                ),
            }
            let _ = app2.emit(
                "game:exit",
                json!({
                    "instanceId": inst_id2,
                    "code": code,
                }),
            );
        };

        let app3 = app_clone.clone();
        let task_id3 = task_id_clone.clone();
        let on_progress = move |p: launcher_launch::LaunchProgress| {
            let _ = app3.emit(
                "task:progress",
                json!({
                    "taskId": task_id3,
                    "progress": {
                        "stage": p.stage,
                        "detail": p.detail,
                        "completed": p.completed,
                        "total": p.total,
                        "stageIndex": p.stage_index,
                        "stageCount": p.stage_count,
                    }
                }),
            );
        };

        match launch_service
            .launch_with_progress(
                &inst_id_clone,
                &version_id,
                server_address.as_deref(),
                on_log,
                on_exit,
                on_progress,
                cancelled,
            )
            .await
        {
            Ok(pid) => {
                logs.record_entry(
                    "stdout",
                    format!("Game berhasil diluncurkan (PID {}).", pid),
                );
                if let Some((name, ver, loader, icon_url)) = instance_info {
                    discord.set_playing(name, ver, loader, icon_url);
                }
                let _ = app_clone.emit(
                    "task:done",
                    json!({
                        "taskId": task_id_clone,
                        "result": { "pid": pid }
                    }),
                );
            }
            Err(e) => {
                logs.record_entry(
                    "stderr",
                    format!("Peluncuran gagal ({:?}): {}", e.code, e.message),
                );
                let _ = app_clone.emit("task:error", super::task_error_payload(&task_id_clone, &e));
            }
        }
    });

    Ok(json!({ "taskId": task_id }))
}

pub async fn kill_instance_and_update_discord(
    launch: &launcher_launch::LaunchService,
    discord: &launcher_discord::DiscordRpcService,
    instance_id: &str,
) -> Result<(), LauncherErrorPayload> {
    let res = launch.kill(instance_id).await;
    if res.is_ok() {
        discord.set_idle();
    }
    res
}

#[tauri::command]
pub async fn launch_kill(
    state: tauri::State<'_, AppState>,
    instance_id: String,
) -> Result<(), LauncherErrorPayload> {
    kill_instance_and_update_discord(&state.launch, &state.discord, &instance_id).await
}

#[tauri::command]
pub async fn tasks_cancel(task_id: String) -> Result<(), LauncherErrorPayload> {
    if let Ok(tasks) = active_tasks().lock() {
        if let Some(c) = tasks.get(&task_id) {
            c.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_n12_register_and_cancel_task() {
        let tid = format!("test-task-{}", Uuid::new_v4());
        let flag = register_task(&tid);
        assert!(!flag.load(Ordering::Relaxed));

        tasks_cancel(tid.clone()).await.unwrap();
        assert!(
            flag.load(Ordering::Relaxed),
            "tasks_cancel must flip registered task's AtomicBool"
        );

        let guard = ActiveTaskGuard::new(tid.clone());
        drop(guard);
        assert!(
            !active_tasks().lock().unwrap().contains_key(&tid),
            "ActiveTaskGuard drop must remove task from active_tasks"
        );
    }

    #[tokio::test]
    async fn test_n31_kill_failure_does_not_set_discord_idle() {
        let root = std::env::temp_dir().join(format!("mcl-n31-test-{}", Uuid::new_v4()));
        let paths = launcher_platform::DataPaths::new(&root);
        let versions = launcher_version::VersionService::new(paths.clone());
        let loaders = launcher_loader::LoaderService::new(paths.clone(), versions.clone());
        let downloader = launcher_download::DownloadManager::new(1);
        let java = launcher_java::JavaService::new(paths.clone());
        let settings = launcher_storage::SettingsStore::new(paths.clone());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let accounts = launcher_storage::AccountsStore::new(paths.clone());
        let secrets = launcher_platform::SecretStore::new(paths.secrets_file.clone());
        let launch = launcher_launch::LaunchService::new(
            paths, versions, loaders, downloader, java, settings, instances, accounts, secrets,
        );
        let discord = launcher_discord::DiscordRpcService::new(launcher_types::DiscordSettings {
            enabled: false,
            show_idle: true,
            show_playing: true,
            custom_client_id: None,
        });

        let before = discord.idle_update_count();
        let res = kill_instance_and_update_discord(&launch, &discord, "nonexistent-instance").await;
        assert!(res.is_err(), "Killing an inactive instance should error");
        assert_eq!(
            discord.idle_update_count(),
            before,
            "Failed kill must not reset Discord RPC to idle"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
