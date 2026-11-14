pub mod args;
pub mod classpath;
pub mod natives;
pub mod process;
pub mod service;

pub use args::*;
pub use classpath::*;
pub use natives::*;
pub use process::*;
pub use service::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_running_game_kill_deadlock() {
        #[cfg(windows)]
        let exe = Path::new("cmd.exe");
        #[cfg(windows)]
        let args = vec!["/c".to_string(), "ping -n 10 127.0.0.1".to_string()];

        #[cfg(not(windows))]
        let exe = Path::new("sleep");
        #[cfg(not(windows))]
        let args = vec!["10".to_string()];

        let running_game = spawn_game_process(exe, &args, Path::new("."), |_| {}, |_| {})
            .await
            .expect("Failed to spawn process");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Wait must release the child mutex so kill() cannot deadlock.
        let kill_res = timeout(Duration::from_secs(2), running_game.kill()).await;
        assert!(
            kill_res.is_ok(),
            "DEADLOCK: running_game.kill() timed out because mutex is locked by wait()"
        );
    }

    #[cfg(windows)]
    #[repr(C)]
    struct ProcessMemoryCountersEx {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }

    #[cfg(windows)]
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn GetProcessHandleCount(h_process: isize, count: *mut u32) -> i32;
    }

    #[cfg(windows)]
    #[link(name = "psapi")]
    extern "system" {
        fn GetProcessMemoryInfo(
            process: isize,
            counters: *mut ProcessMemoryCountersEx,
            cb: u32,
        ) -> i32;
    }

    #[tokio::test]
    async fn test_soak_50_cycles_launch_stop() {
        #[cfg(windows)]
        unsafe fn sample_metrics() -> (f64, f64, u32) {
            let proc = GetCurrentProcess();
            let mut mem = ProcessMemoryCountersEx {
                cb: std::mem::size_of::<ProcessMemoryCountersEx>() as u32,
                page_fault_count: 0,
                peak_working_set_size: 0,
                working_set_size: 0,
                quota_peak_paged_pool_usage: 0,
                quota_paged_pool_usage: 0,
                quota_peak_non_paged_pool_usage: 0,
                quota_non_paged_pool_usage: 0,
                pagefile_usage: 0,
                peak_pagefile_usage: 0,
                private_usage: 0,
            };
            GetProcessMemoryInfo(proc, &mut mem, mem.cb);
            let mut handles: u32 = 0;
            GetProcessHandleCount(proc, &mut handles);
            let private_mb = mem.private_usage as f64 / (1024.0 * 1024.0);
            let ws_mb = mem.working_set_size as f64 / (1024.0 * 1024.0);
            (private_mb, ws_mb, handles)
        }

        #[cfg(not(windows))]
        unsafe fn sample_metrics() -> (f64, f64, u32) {
            (0.0, 0.0, 0)
        }

        let (start_priv, start_ws, start_handles) = unsafe { sample_metrics() };
        println!("\n=== SOAK TEST: 50 CYCLES OF INSTALL, LAUNCH, STOP ===");
        println!(
            "Cycle  0 (Start)   : Private: {:.2} MB | WS: {:.2} MB | Handles: {}",
            start_priv, start_ws, start_handles
        );

        for cycle in 1..=50 {
            let temp_dir = tempfile::tempdir().expect("tempdir");
            let inst_cfg = temp_dir.path().join("instance.json");
            std::fs::write(&inst_cfg, b"{\"id\":\"soak-test\",\"name\":\"Soak\"}").expect("write");
            std::fs::create_dir_all(temp_dir.path().join("mods")).expect("mkdir");

            #[cfg(windows)]
            let exe = Path::new("ping.exe");
            #[cfg(windows)]
            let args = vec!["-n".to_string(), "10".to_string(), "127.0.0.1".to_string()];
            #[cfg(not(windows))]
            let exe = Path::new("sleep");
            #[cfg(not(windows))]
            let args = vec!["10".to_string()];

            let running_game = spawn_game_process(exe, &args, temp_dir.path(), |_| {}, |_| {})
                .await
                .expect("Failed to spawn process");

            tokio::time::sleep(Duration::from_millis(25)).await;

            let _ = running_game.kill().await;
            tokio::time::sleep(Duration::from_millis(25)).await;

            if cycle == 25 {
                let (mid_priv, mid_ws, mid_handles) = unsafe { sample_metrics() };
                println!(
                    "Cycle 25 (Mid-run) : Private: {:.2} MB | WS: {:.2} MB | Handles: {}",
                    mid_priv, mid_ws, mid_handles
                );
            }
        }

        let (end_priv, end_ws, end_handles) = unsafe { sample_metrics() };
        println!(
            "Cycle 50 (End)     : Private: {:.2} MB | WS: {:.2} MB | Handles: {}",
            end_priv, end_ws, end_handles
        );
        println!(
            "Delta (End - Start): Private: {:+.2} MB | WS: {:+.2} MB | Handles: {:+}",
            end_priv - start_priv,
            end_ws - start_ws,
            end_handles as i32 - start_handles as i32
        );

        #[cfg(windows)]
        {
            let handle_delta = (end_handles as i32 - start_handles as i32).abs();
            assert!(
                handle_delta < 30,
                "Handle leak detected: handle count grew by {} across 50 cycles",
                handle_delta
            );
        }
    }

    #[test]
    fn test_n07_memory_override_and_jvm_args_helpers() {
        let mut settings = launcher_types::Settings::default();
        settings.memory.default_min_mb = 512;
        settings.memory.default_max_mb = 2048;
        settings.java.jvm_preset = launcher_types::JvmPreset::LowMemory;
        settings.java.custom_jvm_args = "-DglobalProp=1".to_string();

        let mut inst = launcher_types::Instance {
            schema_version: 1,
            id: "inst-1".to_string(),
            name: "Test".to_string(),
            minecraft_version: "1.20.4".to_string(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Vanilla,
                version: None,
            },
            java: launcher_types::InstanceJavaSettings::default(),
            memory: launcher_types::InstanceMemory {
                override_global: false,
                min_mb: Some(128),
                max_mb: Some(8192),
            },
            jvm_args: launcher_types::InstanceJvmArgs {
                preset: launcher_types::JvmPreset::Performance,
                extra: vec!["-DinstanceExtra=yes".to_string(), "-Xmx9G".to_string()],
            },
            content: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_played_at: None,
            icon_url: None,
        };

        assert_eq!(
            crate::service::compute_memory_bounds(&inst, &settings),
            (512, 2048)
        );
        inst.memory.override_global = true;
        assert_eq!(
            crate::service::compute_memory_bounds(&inst, &settings),
            (256, 8192)
        );

        let args_perf = crate::service::collect_jvm_args(&inst, &settings);
        assert!(args_perf.contains(&"-XX:+UseG1GC".to_string()));
        assert!(args_perf.contains(&"-DinstanceExtra=yes".to_string()));
        assert!(!args_perf.contains(&"-Xmx9G".to_string()));

        inst.jvm_args.preset = launcher_types::JvmPreset::Default;
        let args_inherited = crate::service::collect_jvm_args(&inst, &settings);
        assert!(args_inherited.contains(&"-XX:+UseSerialGC".to_string()));
        assert!(args_inherited.contains(&"-DinstanceExtra=yes".to_string()));
    }
}
