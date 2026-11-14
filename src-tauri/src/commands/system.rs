use launcher_platform::get_system_memory;
use launcher_types::{LauncherErrorPayload, MemoryInfo};

#[tauri::command]
pub fn system_memory_info() -> Result<MemoryInfo, LauncherErrorPayload> {
    Ok(get_system_memory())
}
