use crate::state::AppState;
use launcher_types::{JavaInstallation, LauncherErrorPayload};
use serde_json::json;
use tauri::Emitter;

#[tauri::command]
pub fn java_detect(
    state: tauri::State<AppState>,
) -> Result<Vec<JavaInstallation>, LauncherErrorPayload> {
    Ok(state.java.detect())
}

#[tauri::command]
pub fn java_validate(
    state: tauri::State<AppState>,
    path: String,
) -> Result<JavaInstallation, LauncherErrorPayload> {
    state.java.validate(&path)
}

#[tauri::command]
pub async fn java_ensure(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    major: u32,
) -> Result<serde_json::Value, LauncherErrorPayload> {
    let task_id = uuid::Uuid::new_v4().to_string();
    let task_id_clone = task_id.clone();
    let app_clone = app.clone();
    let java_service = state.java.clone();
    let downloader = state.downloader.clone();
    let cancelled = crate::commands::launch::register_task(&task_id);

    tokio::spawn(async move {
        let _guard = crate::commands::launch::ActiveTaskGuard::new(task_id_clone.clone());
        let app_prog = app_clone.clone();
        let tid_prog = task_id_clone.clone();
        let progress_cb = move |p: launcher_download::DownloadProgress| {
            let _ = app_prog.emit(
                "task:progress",
                json!({
                    "taskId": tid_prog,
                    "progress": {
                        "stage": "Mengunduh Java Runtime",
                        "completed": p.completed_bytes,
                        "total": p.total_bytes,
                    }
                }),
            );
        };

        match java_service
            .ensure(
                major,
                None,
                &downloader,
                progress_cb,
                std::sync::Arc::clone(&cancelled),
            )
            .await
        {
            Ok(inst) => {
                let _ = app_clone.emit(
                    "task:done",
                    json!({
                        "taskId": task_id_clone,
                        "result": inst,
                    }),
                );
            }
            Err(e) => {
                let _ = app_clone.emit("task:error", java_task_error_payload(&task_id_clone, &e));
            }
        }
    });

    Ok(json!({ "taskId": task_id }))
}

fn java_task_error_payload(task_id: &str, e: &LauncherErrorPayload) -> serde_json::Value {
    super::task_error_payload(task_id, e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_types::ErrorCode;

    #[test]
    fn test_n19_java_task_error_code_screaming_snake_case() {
        let err = LauncherErrorPayload::new(ErrorCode::JavaMissing, "no java");
        let payload = java_task_error_payload("t1", &err);
        assert_eq!(payload["error"]["code"], "JAVA_MISSING");
    }
}
