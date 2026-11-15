use crate::state::AppState;
use launcher_types::{
    ContentFilter, ContentInstallPlan, ContentProject, ContentProviderId, ContentUpdateInfo,
    ContentVersion, InstallModpackRequest, Instance, LauncherErrorPayload, SavedWorld, SearchQuery,
    SearchResult,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{Emitter, State};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProjectRequest {
    pub provider: ContentProviderId,
    pub project_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentVersionsRequest {
    pub provider: ContentProviderId,
    pub project_id: String,
    pub filter: ContentFilter,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentPlanRequest {
    pub instance_id: String,
    pub provider: ContentProviderId,
    pub version_id: String,
    pub world_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentInstallRequest {
    pub instance_id: String,
    pub provider: ContentProviderId,
    pub version_id: String,
    pub world_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRemoveRequest {
    pub instance_id: String,
    pub provider: ContentProviderId,
    pub project_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentToggleRequest {
    pub instance_id: String,
    pub provider: ContentProviderId,
    pub project_id: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackTaskResponse {
    pub task_id: String,
}

#[tauri::command]
pub async fn content_search(
    state: State<'_, AppState>,
    req: SearchQuery,
) -> Result<SearchResult, LauncherErrorPayload> {
    state.content.search(req).await
}

#[tauri::command]
pub async fn content_project(
    state: State<'_, AppState>,
    req: ContentProjectRequest,
) -> Result<ContentProject, LauncherErrorPayload> {
    state.content.project(req.provider, &req.project_id).await
}

#[tauri::command]
pub async fn content_versions(
    state: State<'_, AppState>,
    req: ContentVersionsRequest,
) -> Result<Vec<ContentVersion>, LauncherErrorPayload> {
    state
        .content
        .versions(req.provider, &req.project_id, req.filter)
        .await
}

#[tauri::command]
pub async fn content_plan(
    state: State<'_, AppState>,
    req: ContentPlanRequest,
) -> Result<ContentInstallPlan, LauncherErrorPayload> {
    state
        .content
        .plan_with_world(
            &req.instance_id,
            req.provider,
            &req.version_id,
            req.world_id.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn content_install(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    req: ContentInstallRequest,
) -> Result<ModpackTaskResponse, LauncherErrorPayload> {
    let content = state.content.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let task_id_clone = task_id.clone();
    let cancel_flag = crate::commands::launch::register_task(&task_id);

    tokio::spawn(async move {
        let _guard = crate::commands::launch::ActiveTaskGuard::new(task_id_clone.clone());
        match content
            .install_with_cancel(
                &req.instance_id,
                req.provider,
                &req.version_id,
                req.world_id.as_deref(),
                cancel_flag,
            )
            .await
        {
            Ok(inst) => {
                let _ = app.emit(
                    "task:done",
                    json!({
                        "taskId": task_id_clone,
                        "result": inst,
                    }),
                );
            }
            Err(e) => {
                let _ = app.emit("task:error", content_task_error_payload(&task_id_clone, &e));
            }
        }
    });

    Ok(ModpackTaskResponse { task_id })
}

#[tauri::command]
pub fn content_remove(
    state: State<'_, AppState>,
    req: ContentRemoveRequest,
) -> Result<Instance, LauncherErrorPayload> {
    state
        .content
        .remove(&req.instance_id, req.provider, &req.project_id)
}

#[tauri::command]
pub fn content_toggle(
    state: State<'_, AppState>,
    req: ContentToggleRequest,
) -> Result<Instance, LauncherErrorPayload> {
    state
        .content
        .toggle(&req.instance_id, req.provider, &req.project_id, req.enabled)
}

#[tauri::command]
pub fn content_worlds(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<SavedWorld>, LauncherErrorPayload> {
    state.content.worlds(&instance_id)
}

#[tauri::command]
pub async fn content_check_updates(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<ContentUpdateInfo>, LauncherErrorPayload> {
    state.content.check_updates(&instance_id).await
}

#[tauri::command]
pub async fn content_install_modpack(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    req: InstallModpackRequest,
) -> Result<ModpackTaskResponse, LauncherErrorPayload> {
    let content = state.content.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let task_id_clone = task_id.clone();
    let app_clone = app.clone();
    let task_id_prog = task_id.clone();
    let cancel_flag = crate::commands::launch::register_task(&task_id);

    let on_progress = move |stage: &str, completed: u32, total: u32| {
        let _ = app_clone.emit(
            "task:progress",
            json!({
                "taskId": task_id_prog,
                "progress": {
                    "stage": stage,
                    "completed": completed,
                    "total": total,
                    "stageIndex": completed.saturating_sub(1),
                    "stageCount": total,
                }
            }),
        );
    };

    tokio::spawn(async move {
        let _guard = crate::commands::launch::ActiveTaskGuard::new(task_id_clone.clone());
        let options = launcher_content::modpack::ModpackInstallOptions {
            project_id: req.project_id.clone(),
            icon_url: req.icon_url.clone(),
        };
        match content
            .install_modpack_with_options(
                req.provider,
                &req.version_id,
                req.instance_name.as_deref(),
                options,
                on_progress,
                cancel_flag,
            )
            .await
        {
            Ok(inst) => {
                let _ = app.emit(
                    "task:done",
                    json!({
                        "taskId": task_id_clone,
                        "result": inst,
                    }),
                );
            }
            Err(e) => {
                let _ = app.emit("task:error", content_task_error_payload(&task_id_clone, &e));
            }
        }
    });

    Ok(ModpackTaskResponse { task_id })
}

fn content_task_error_payload(task_id: &str, e: &LauncherErrorPayload) -> serde_json::Value {
    super::task_error_payload(task_id, e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_types::ErrorCode;

    #[test]
    fn test_n19_content_task_error_code_screaming_snake_case() {
        let err = LauncherErrorPayload::new(ErrorCode::DistributionBlocked, "blocked");
        let payload = content_task_error_payload("t1", &err);
        assert_eq!(payload["error"]["code"], "DISTRIBUTION_BLOCKED");

        let net_err = LauncherErrorPayload::new(ErrorCode::RateLimited, "offline");
        let payload2 = content_task_error_payload("t2", &net_err);
        assert_eq!(payload2["error"]["code"], "RATE_LIMITED");
    }
}
