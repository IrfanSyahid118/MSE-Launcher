pub mod accounts;
pub mod content;
pub mod downloads;
pub mod instances;
pub mod integrations;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod logs;
pub mod servers;
pub mod settings;
pub mod shell;
pub mod system;
pub mod updater;
pub mod versions;

pub use accounts::*;
pub use content::*;
pub use downloads::*;
pub use instances::*;
pub use integrations::*;
pub use java::*;
pub use launch::*;
pub use loaders::*;
pub use logs::*;
pub use servers::*;
pub use settings::*;
pub use shell::*;
pub use system::*;
pub use updater::*;
pub use versions::*;

pub(crate) fn task_error_payload(
    task_id: &str,
    e: &launcher_types::LauncherErrorPayload,
) -> serde_json::Value {
    serde_json::json!({
        "taskId": task_id,
        "error": {
            "code": serde_json::to_value(e.code).unwrap_or_else(|_| serde_json::json!("INTERNAL")),
            "message": e.message,
            "retryable": false,
        }
    })
}
