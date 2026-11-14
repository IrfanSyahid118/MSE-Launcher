use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use launcher_launch::LaunchService;

#[tokio::test]
async fn test_n06_concurrent_launch_same_instance_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let paths = launcher_platform::DataPaths::new(temp.path());
    let instances = launcher_storage::InstancesStore::new(paths.clone());
    let accounts = launcher_storage::AccountsStore::new(paths.clone());
    let settings = launcher_storage::SettingsStore::new(paths.clone());
    let versions = launcher_version::VersionService::new(paths.clone());
    let loaders = launcher_loader::LoaderService::new(paths.clone(), versions.clone());
    let downloader = launcher_download::DownloadManager::new(2);
    let java = launcher_java::JavaService::new(paths.clone());

    let inst = instances
        .create(launcher_types::CreateInstanceRequest {
            name: "Race Test".into(),
            minecraft_version: "1.20.4".into(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Vanilla,
                version: None,
            },
        })
        .unwrap();
    let acc = accounts.create_offline("RacePlayer").unwrap();
    settings
        .set(launcher_types::SettingsPatch {
            active_account_id: Some(Some(acc.id)),
            theme: None,
            language: None,
            memory: None,
            java: None,
            downloads: None,
            discord: None,
        })
        .unwrap();

    let hit_count = Arc::new(AtomicUsize::new(0));
    let hit_counter = Arc::clone(&hit_count);
    let stop_server = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop_server);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let srv_handle = std::thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            if stop_flag.load(Ordering::SeqCst) {
                break;
            }
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            hit_counter.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(250));
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });

    let ver_dir = paths.versions_dir.join("1.20.4");
    std::fs::create_dir_all(&ver_dir).unwrap();
    std::fs::write(
        ver_dir.join("1.20.4.json"),
        format!(
            r#"{{"id":"1.20.4","mainClass":"net.minecraft.client.main.Main","downloads":{{"client":{{"sha1":"","size":0,"url":"http://127.0.0.1:{}/client.jar"}}}},"libraries":[]}}"#,
            port
        ),
    )
    .unwrap();

    let secrets = launcher_platform::SecretStore::new(paths.secrets_file.clone());
    let service = LaunchService::new(
        paths, versions, loaders, downloader, java, settings, instances, accounts, secrets,
    );

    let c1 = Arc::new(AtomicBool::new(false));
    let c2 = Arc::new(AtomicBool::new(false));
    let (r1, r2) = tokio::join!(
        service.launch(&inst.id, "1.20.4", None, |_| {}, |_| {}, |_, _, _| {}, c1),
        service.launch(&inst.id, "1.20.4", None, |_| {}, |_| {}, |_, _, _| {}, c2),
    );

    let err1 = r1.unwrap_err().message;
    let err2 = r2.unwrap_err().message;
    stop_server.store(true, Ordering::SeqCst);
    let _ = std::net::TcpStream::connect(format!("127.0.0.1:{}", port));
    let _ = srv_handle.join();

    assert!(
        err1.contains("sudah berjalan") || err2.contains("sudah berjalan"),
        "Second concurrent launch must be rejected immediately as already running! err1={err1:?}, err2={err2:?}"
    );
    assert_eq!(
        hit_count.load(Ordering::SeqCst),
        1,
        "Only one launch should reach download stage"
    );
}

#[tokio::test]
async fn test_n07_launch_respects_java_memory_and_jvm_settings() {
    let temp = tempfile::tempdir().unwrap();
    let paths = launcher_platform::DataPaths::new(temp.path());
    let instances = launcher_storage::InstancesStore::new(paths.clone());
    let accounts = launcher_storage::AccountsStore::new(paths.clone());
    let settings = launcher_storage::SettingsStore::new(paths.clone());
    let versions = launcher_version::VersionService::new(paths.clone());
    let loaders = launcher_loader::LoaderService::new(paths.clone(), versions.clone());
    let downloader = launcher_download::DownloadManager::new(2);
    let java = launcher_java::JavaService::new(paths.clone());

    let java_bin = if cfg!(windows) { "java.exe" } else { "java" };
    let acc = accounts.create_offline("SettingsPlayer").unwrap();
    let global_missing_java = temp
        .path()
        .join("global_custom/bin")
        .join(java_bin)
        .to_string_lossy()
        .to_string();
    settings
        .set(launcher_types::SettingsPatch {
            active_account_id: Some(Some(acc.id)),
            theme: None,
            language: None,
            memory: Some(launcher_types::MemorySettingsPatch {
                default_min_mb: Some(512),
                default_max_mb: Some(2048),
            }),
            java: Some(launcher_types::JavaSettingsPatch {
                prefer_managed: Some(false),
                custom_path: Some(Some(global_missing_java)),
                jvm_preset: Some(launcher_types::JvmPreset::LowMemory),
                custom_jvm_args: Some("-DglobalProp=1".to_string()),
            }),
            downloads: None,
            discord: None,
        })
        .unwrap();

    let ver_dir = paths.versions_dir.join("1.20.4");
    std::fs::create_dir_all(&ver_dir).unwrap();
    std::fs::write(
        ver_dir.join("1.20.4.json"),
        r#"{"id":"1.20.4","mainClass":"net.minecraft.client.main.Main","libraries":[],"javaVersion":{"majorVersion":99}}"#,
    )
    .unwrap();

    let mut inst_a = instances
        .create(launcher_types::CreateInstanceRequest {
            name: "Settings Subcase A".into(),
            minecraft_version: "1.20.4".into(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Vanilla,
                version: None,
            },
        })
        .unwrap();
    inst_a.java = launcher_types::InstanceJavaSettings {
        mode: launcher_types::InstanceJavaMode::Auto,
        path: Some(
            temp.path()
                .join("stale_ignored/bin")
                .join(java_bin)
                .to_string_lossy()
                .to_string(),
        ),
    };
    instances.update(inst_a.clone()).unwrap();

    let secrets = launcher_platform::SecretStore::new(paths.secrets_file.clone());
    let service = LaunchService::new(
        paths.clone(),
        versions,
        loaders,
        downloader,
        java,
        settings.clone(),
        instances.clone(),
        accounts,
        secrets,
    );

    let err_a = timeout(
        Duration::from_secs(3),
        service.launch(
            &inst_a.id,
            "1.20.4",
            None,
            |_| {},
            |_| {},
            |_, _, _| {},
            Arc::new(AtomicBool::new(false)),
        ),
    )
    .await
    .expect("Subcase A must fail fast on global custom_path without hitting network")
    .unwrap_err();
    assert!(
        err_a.message.contains("global_custom"),
        "Expected validation error for settings.java.custom_path, got: {}",
        err_a.message
    );

    let mut inst_b = instances
        .create(launcher_types::CreateInstanceRequest {
            name: "Settings Subcase B".into(),
            minecraft_version: "1.20.4".into(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Vanilla,
                version: None,
            },
        })
        .unwrap();
    inst_b.java = launcher_types::InstanceJavaSettings {
        mode: launcher_types::InstanceJavaMode::Custom,
        path: Some(
            temp.path()
                .join("missing_custom")
                .join(java_bin)
                .to_string_lossy()
                .to_string(),
        ),
    };
    instances.update(inst_b.clone()).unwrap();
    let err_b = service
        .launch(
            &inst_b.id,
            "1.20.4",
            None,
            |_| {},
            |_| {},
            |_, _, _| {},
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();
    assert!(
        err_b.message.contains("missing_custom"),
        "Custom Java mode must validate instance.java.path without fallback, got: {}",
        err_b.message
    );

    settings
        .set(launcher_types::SettingsPatch {
            active_account_id: None,
            theme: None,
            language: None,
            memory: None,
            java: Some(launcher_types::JavaSettingsPatch {
                prefer_managed: Some(false),
                custom_path: Some(None),
                jvm_preset: None,
                custom_jvm_args: None,
            }),
            downloads: None,
            discord: None,
        })
        .unwrap();
    let inst_c = instances
        .create(launcher_types::CreateInstanceRequest {
            name: "Settings Subcase C".into(),
            minecraft_version: "1.20.4".into(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Vanilla,
                version: None,
            },
        })
        .unwrap();
    let err_c = timeout(
        Duration::from_secs(3),
        service.launch(
            &inst_c.id,
            "1.20.4",
            None,
            |_| {},
            |_| {},
            |_, _, _| {},
            Arc::new(AtomicBool::new(false)),
        ),
    )
    .await
    .expect("prefer_managed=false must not hit network ensure()")
    .unwrap_err();
    assert!(
        err_c.message.contains("dinonaktifkan") || err_c.message.contains("Java 99"),
        "Unexpected error for prefer_managed=false: {}",
        err_c.message
    );

    settings
        .set(launcher_types::SettingsPatch {
            active_account_id: None,
            theme: None,
            language: None,
            memory: None,
            java: Some(launcher_types::JavaSettingsPatch {
                prefer_managed: Some(true),
                custom_path: Some(None),
                jvm_preset: None,
                custom_jvm_args: None,
            }),
            downloads: None,
            discord: None,
        })
        .unwrap();
    let cancel_d = Arc::new(AtomicBool::new(false));
    let cancel_d_cb = Arc::clone(&cancel_d);
    let res_d = timeout(
        Duration::from_secs(3),
        service.launch(
            &inst_c.id,
            "1.20.4",
            None,
            |_| {},
            |_| {},
            move |_, step, _| {
                if step >= 4 {
                    cancel_d_cb.store(true, Ordering::Relaxed);
                }
            },
            cancel_d,
        ),
    )
    .await
    .expect("Subcase D should fail fast when cancelled during step 4");
    assert!(
        res_d.is_err(),
        "Cancelled managed Java download must error without arbitrary fallback"
    );
}

#[tokio::test]
async fn test_n08_expired_microsoft_token_refresh_error_aborts_launch() {
    let temp = tempfile::tempdir().unwrap();
    let paths = launcher_platform::DataPaths::new(temp.path());
    let instances = launcher_storage::InstancesStore::new(paths.clone());
    let accounts = launcher_storage::AccountsStore::new(paths.clone());
    let settings = launcher_storage::SettingsStore::new(paths.clone());
    let versions = launcher_version::VersionService::new(paths.clone());
    let loaders = launcher_loader::LoaderService::new(paths.clone(), versions.clone());
    let downloader = launcher_download::DownloadManager::new(2);
    let java = launcher_java::JavaService::new(paths.clone());

    let inst = instances
        .create(launcher_types::CreateInstanceRequest {
            name: "Auth Refresh Test".into(),
            minecraft_version: "1.20.4".into(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Vanilla,
                version: None,
            },
        })
        .unwrap();

    let ms_acc = launcher_types::Account {
        id: "ms-expired-1".to_string(),
        username: "ExpiredPlayer".to_string(),
        uuid: "00000000-0000-0000-0000-000000000001".to_string(),
        account_type: launcher_types::AccountType::Microsoft,
        created_at: "2020-01-01T00:00:00Z".to_string(),
        expires_at: Some("2020-01-01T00:00:00Z".to_string()),
        tokens: None,
        xuid: None,
        skin_url: None,
        skin_variant: None,
        cape_url: None,
        active_cape_id: None,
        capes: Vec::new(),
    };
    accounts.save_account(ms_acc.clone()).unwrap();

    let secrets = launcher_platform::SecretStore::new(paths.secrets_file.clone());
    secrets
        .set(
            "tokens:ms-expired-1",
            r#"{"access_token":"expired_tok","refresh_token":"revoked_rt"}"#,
        )
        .await
        .unwrap();

    let ver_dir = paths.versions_dir.join("1.20.4");
    std::fs::create_dir_all(&ver_dir).unwrap();
    std::fs::write(
        ver_dir.join("1.20.4.json"),
        r#"{"id":"1.20.4","mainClass":"net.minecraft.client.main.Main","libraries":[]}"#,
    )
    .unwrap();

    settings
        .set(launcher_types::SettingsPatch {
            active_account_id: Some(Some(ms_acc.id)),
            theme: None,
            language: None,
            memory: None,
            java: Some(launcher_types::JavaSettingsPatch {
                prefer_managed: Some(false),
                custom_path: Some(Some(
                    temp.path()
                        .join("missing_java_sentinel")
                        .join(if cfg!(windows) { "java.exe" } else { "java" })
                        .to_string_lossy()
                        .to_string(),
                )),
                jvm_preset: None,
                custom_jvm_args: None,
            }),
            downloads: None,
            discord: None,
        })
        .unwrap();

    let service = LaunchService::new(
        paths, versions, loaders, downloader, java, settings, instances, accounts, secrets,
    );

    let err = service
        .launch(
            &inst.id,
            "1.20.4",
            None,
            |_| {},
            |_| {},
            |_, _, _| {},
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            err.code,
            launcher_types::ErrorCode::AuthFailed
                | launcher_types::ErrorCode::AuthExpired
                | launcher_types::ErrorCode::ProviderNotConfigured
                | launcher_types::ErrorCode::Network
        ),
        "Expired token refresh error must abort launch before version/download step, instead got {:?}: {}",
        err.code,
        err.message
    );
}

#[tokio::test]
async fn test_n17_skin_injection_respects_account_type_and_no_forced_csl_download() {
    let temp = tempfile::tempdir().unwrap();
    let paths = launcher_platform::DataPaths::new(temp.path());
    let instances = launcher_storage::InstancesStore::new(paths.clone());
    let accounts = launcher_storage::AccountsStore::new(paths.clone());
    let settings = launcher_storage::SettingsStore::new(paths.clone());
    let versions = launcher_version::VersionService::new(paths.clone());
    let loaders = launcher_loader::LoaderService::new(paths.clone(), versions.clone());
    let downloader = launcher_download::DownloadManager::new(2);
    let java = launcher_java::JavaService::new(paths.clone());
    let secrets = launcher_platform::SecretStore::new(paths.secrets_file.clone());

    let inst = instances
        .create(launcher_types::CreateInstanceRequest {
            name: "N17 Test".into(),
            minecraft_version: "1.21.4".into(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Fabric,
                version: Some("0.16.0".into()),
            },
        })
        .unwrap();

    let service = LaunchService::new(
        paths.clone(),
        versions,
        loaders,
        downloader,
        java,
        settings,
        instances,
        accounts,
        secrets,
    );

    let cancel = Arc::new(AtomicBool::new(false));

    let ms_game_dir = paths.instances_dir.join(&inst.id).join("ms-minecraft");
    std::fs::create_dir_all(&ms_game_dir).unwrap();
    let ms_acc = launcher_types::Account {
        id: "ms-n17".into(),
        username: "OfficialUser".into(),
        uuid: "5627dd98-e6be-3c21-b8a8-e92344183641".into(),
        account_type: launcher_types::AccountType::Microsoft,
        created_at: "2026-01-01T00:00:00Z".into(),
        expires_at: None,
        tokens: None,
        xuid: None,
        skin_url: Some("data:image/png;base64,iVBORw0KGgo=".into()),
        skin_variant: Some("classic".into()),
        cape_url: None,
        active_cape_id: None,
        capes: Vec::new(),
    };
    service
        .prepare_skin_artifacts(&inst, &ms_acc, &ms_game_dir, &cancel)
        .await
        .unwrap();
    assert!(
        !ms_game_dir.join("options.txt").exists(),
        "Microsoft account launch must not modify options.txt"
    );
    assert!(
        !ms_game_dir
            .join("resourcepacks")
            .join("SkinLoader-ResourcePack.zip")
            .exists(),
        "Microsoft account launch must not create SkinLoader-ResourcePack.zip"
    );
    assert!(
        !ms_game_dir.join("CustomSkinLoader").exists(),
        "Microsoft account launch must not create CustomSkinLoader directory"
    );
    assert!(
        !ms_game_dir
            .join("mods")
            .join("CustomSkinLoader_Universal-15.1.jar")
            .exists(),
        "Microsoft account launch must not download CustomSkinLoader JAR into mods/"
    );

    let offline_no_skin_dir = paths.instances_dir.join(&inst.id).join("off-noskin");
    std::fs::create_dir_all(&offline_no_skin_dir).unwrap();
    let off_default = launcher_types::Account {
        id: "off-default".into(),
        username: "PlainOffline".into(),
        uuid: "5627dd98-e6be-3c21-b8a8-e92344183642".into(),
        account_type: launcher_types::AccountType::Offline,
        created_at: "2026-01-01T00:00:00Z".into(),
        expires_at: None,
        tokens: None,
        xuid: None,
        skin_url: None,
        skin_variant: None,
        cape_url: None,
        active_cape_id: None,
        capes: Vec::new(),
    };
    service
        .prepare_skin_artifacts(&inst, &off_default, &offline_no_skin_dir, &cancel)
        .await
        .unwrap();
    assert!(
        !offline_no_skin_dir.join("options.txt").exists(),
        "Offline account without custom skin must not rewrite options.txt"
    );
    assert!(
        !offline_no_skin_dir
            .join("mods")
            .join("CustomSkinLoader_Universal-15.1.jar")
            .exists(),
        "Must not force-download CustomSkinLoader JAR into mods/"
    );

    let offline_skin_dir = paths.instances_dir.join(&inst.id).join("off-custom");
    let csl_dir = offline_skin_dir.join("CustomSkinLoader");
    std::fs::create_dir_all(&csl_dir).unwrap();
    let custom_cfg = r#"{"version":"user-custom","loadlist":[]}"#;
    std::fs::write(csl_dir.join("CustomSkinLoader.json"), custom_cfg).unwrap();

    let off_custom = launcher_types::Account {
        skin_url: Some("data:image/png;base64,iVBORw0KGgo=".into()),
        ..off_default
    };
    service
        .prepare_skin_artifacts(&inst, &off_custom, &offline_skin_dir, &cancel)
        .await
        .unwrap();
    assert!(offline_skin_dir
        .join("resourcepacks")
        .join("SkinLoader-ResourcePack.zip")
        .exists());
    assert!(offline_skin_dir
        .join("CustomSkinLoader")
        .join("LocalSkin")
        .join("skins")
        .join("PlainOffline.png")
        .exists());
    assert_eq!(
        std::fs::read_to_string(csl_dir.join("CustomSkinLoader.json")).unwrap(),
        custom_cfg,
        "Existing user CustomSkinLoader.json must not be overwritten"
    );
    assert!(
        !offline_skin_dir
            .join("mods")
            .join("CustomSkinLoader_Universal-15.1.jar")
            .exists(),
        "Must not force-download CustomSkinLoader JAR when not installed"
    );
}

#[test]
fn test_n18_quick_connect_args_version_and_ipv6() {
    assert_eq!(
        launcher_launch::build_quick_connect_args("1.20.4", "play.example.com:25565"),
        vec!["--quickPlayMultiplayer", "play.example.com:25565"]
    );
    assert_eq!(
        launcher_launch::build_quick_connect_args("1.20", "[::1]:25565"),
        vec!["--quickPlayMultiplayer", "[::1]:25565"]
    );
    assert_eq!(
        launcher_launch::build_quick_connect_args("23w14a", "mc.example.net"),
        vec!["--quickPlayMultiplayer", "mc.example.net"]
    );

    assert_eq!(
        launcher_launch::build_quick_connect_args("1.19.4", "play.example.com:25565"),
        vec!["--server", "play.example.com", "--port", "25565"]
    );
    assert_eq!(
        launcher_launch::build_quick_connect_args("1.19.4", "[::1]:25565"),
        vec!["--server", "::1", "--port", "25565"]
    );
    assert_eq!(
        launcher_launch::build_quick_connect_args("1.19.4", "[2001:db8::1]"),
        vec!["--server", "2001:db8::1"]
    );
    assert_eq!(
        launcher_launch::build_quick_connect_args("1.12.2", "2001:db8::1"),
        vec!["--server", "2001:db8::1"]
    );
    assert_eq!(
        launcher_launch::build_quick_connect_args("1.12.2", "play.example.com"),
        vec!["--server", "play.example.com"]
    );
}

#[tokio::test]
async fn test_n33_launch_updates_instance_last_played_at() {
    let temp = tempfile::tempdir().unwrap();
    let paths = launcher_platform::DataPaths::new(temp.path());
    let instances = launcher_storage::InstancesStore::new(paths.clone());
    let accounts = launcher_storage::AccountsStore::new(paths.clone());
    let settings = launcher_storage::SettingsStore::new(paths.clone());
    let versions = launcher_version::VersionService::new(paths.clone());
    let loaders = launcher_loader::LoaderService::new(paths.clone(), versions.clone());
    let downloader = launcher_download::DownloadManager::new(2);
    let java = launcher_java::JavaService::new(paths.clone());

    let fake_java = {
        let bin_dir = temp.path().join("mock-jre").join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        #[cfg(windows)]
        {
            let src = temp.path().join("mock_java.rs");
            let exe = bin_dir.join("java.exe");
            std::fs::write(
                &src,
                r#"fn main() {
    if std::env::args().any(|a| a == "-XshowSettings:properties") {
        eprintln!("openjdk version \"21.0.2\" 2024-01-16");
        eprintln!("sun.arch.data.model = 64");
    }
}
"#,
            )
            .unwrap();
            let rustc =
                std::env::var_os("RUSTC").unwrap_or_else(|| std::ffi::OsString::from("rustc"));
            let status = std::process::Command::new(rustc)
                .arg(&src)
                .arg("-o")
                .arg(&exe)
                .status()
                .expect("rustc must compile mock java.exe");
            assert!(status.success(), "rustc failed building mock java.exe");
            exe
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = bin_dir.join("java");
            std::fs::write(
                &p,
                "#!/bin/sh\nif [ \"$1\" = \"-XshowSettings:properties\" ]; then\n  echo 'openjdk version \"21.0.2\" 2024-01-16' >&2\n  echo 'sun.arch.data.model = 64' >&2\nfi\nexit 0\n",
            )
            .unwrap();
            let mut perms = std::fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&p, perms).unwrap();
            p
        }
    };

    let mut inst = instances
        .create(launcher_types::CreateInstanceRequest {
            name: "LastPlayed Test".into(),
            minecraft_version: "1.20.4".into(),
            loader: launcher_types::LoaderSpec {
                loader_type: launcher_types::LoaderType::Vanilla,
                version: None,
            },
        })
        .unwrap();
    assert!(inst.last_played_at.is_none());

    inst.java = launcher_types::InstanceJavaSettings {
        mode: launcher_types::InstanceJavaMode::Custom,
        path: Some(fake_java.to_string_lossy().to_string()),
    };
    instances.update(inst.clone()).unwrap();

    let acc = accounts.create_offline("LastPlayedTester").unwrap();
    settings
        .set(launcher_types::SettingsPatch {
            active_account_id: Some(Some(acc.id)),
            theme: None,
            language: None,
            memory: None,
            java: None,
            downloads: None,
            discord: None,
        })
        .unwrap();

    let ver_dir = paths.versions_dir.join("1.20.4");
    std::fs::create_dir_all(&ver_dir).unwrap();
    std::fs::write(ver_dir.join("1.20.4.jar"), b"dummy-client").unwrap();
    std::fs::write(
        ver_dir.join("1.20.4.json"),
        r#"{"id":"1.20.4","mainClass":"net.minecraft.client.main.Main","javaVersion":{"majorVersion":21},"libraries":[]}"#,
    )
    .unwrap();

    let secrets = launcher_platform::SecretStore::new(paths.secrets_file.clone());
    let service = LaunchService::new(
        paths,
        versions,
        loaders,
        downloader,
        java,
        settings,
        instances.clone(),
        accounts,
        secrets,
    );

    let pid = timeout(
        Duration::from_secs(5),
        service.launch(
            &inst.id,
            "1.20.4",
            None,
            |_| {},
            |_| {},
            |_, _, _| {},
            Arc::new(AtomicBool::new(false)),
        ),
    )
    .await
    .expect("Launch with mock Java and cached offline manifest should complete quickly")
    .expect("Mock launch should succeed");
    assert!(pid > 0);

    let updated = instances.get(&inst.id).unwrap();
    assert!(
        updated.last_played_at.is_some(),
        "LaunchService::launch must update instance.last_played_at on launch"
    );
}
