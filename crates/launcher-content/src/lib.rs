pub mod curseforge;
pub mod modpack;
pub mod modrinth;
pub mod plan;
pub mod service;

pub use service::ContentService;

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_types::ContentType;

    #[test]
    fn test_class_ids() {
        assert_eq!(curseforge::class_id_for_type(ContentType::Mod), 6);
        assert_eq!(curseforge::class_id_for_type(ContentType::Modpack), 4471);
        assert_eq!(curseforge::class_id_for_type(ContentType::Resourcepack), 12);
        assert_eq!(curseforge::class_id_for_type(ContentType::Shader), 6552);
    }

    #[test]
    fn test_loader_mappings() {
        assert_eq!(curseforge::loader_type_id("forge"), Some(1));
        assert_eq!(curseforge::loader_type_id("Fabric"), Some(4));
        assert_eq!(curseforge::loader_type_id("QUILT"), Some(5));
        assert_eq!(curseforge::loader_type_id("NeoForge"), Some(6));
        assert_eq!(curseforge::loader_type_id("vanilla"), None);
    }

    #[test]
    fn test_modpack_overrides_traversal_rejected() {
        let base = std::path::PathBuf::from("C:\\fake\\unpacked");
        let malicious_overrides = "../../windows/system32";
        let res = launcher_security::safe_subpath(&base, malicious_overrides);
        assert!(res.is_err(), "Path traversal in overrides must be rejected");
    }

    #[test]
    fn test_integrations_atomic_write_bug23() {
        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let http_dl = launcher_net::HttpClient::new_loopback_test();
        let downloader = launcher_download::DownloadManager::with_http_client(2, http_dl.clone());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let secrets = launcher_platform::SecretStore::new(paths.secrets_file.clone());
        let service = ContentService::with_http_client(
            paths.clone(),
            http_dl.clone(),
            downloader,
            instances,
            secrets,
        );

        let status = service.set_curseforge_key(Some("cf-key-12345")).unwrap();
        assert!(status.curseforge.source.is_some());
        assert_eq!(
            service.get_curseforge_key().as_deref(),
            Some("cf-key-12345")
        );

        let secrets_file = paths.secrets_file.clone();
        assert!(secrets_file.exists());
        let secret_bytes = std::fs::read(&secrets_file).unwrap();
        assert!(!String::from_utf8_lossy(&secret_bytes).contains("cf-key-12345"));

        let int_file = paths.root.join("integrations.json");
        if int_file.exists() {
            let content = std::fs::read_to_string(&int_file).unwrap();
            assert!(!content.contains("cf-key-12345"));
        }
    }

    #[test]
    fn test_n01b_content_folder_world_traversal_rejected() {
        let mc_dir = std::path::PathBuf::from("C:\\fake\\minecraft");
        let res =
            ContentService::content_folder(&mc_dir, ContentType::Datapack, Some("../../evil"));
        assert!(
            res.is_err(),
            "world_id traversal must be rejected before joining saves path"
        );
    }

    #[test]
    fn test_n01c_modpack_filename_traversal_rejected() {
        let base = std::path::PathBuf::from("C:\\fake\\staging");
        assert!(modpack::safe_modpack_file_path(&base, "../../evil.mrpack").is_err());
        assert!(modpack::safe_modpack_file_path(&base, "sub/evil.jar").is_err());
        assert!(modpack::safe_modpack_file_path(&base, "C:\\evil.jar").is_err());
    }

    fn spawn_mock_http_server(
        routes: Vec<(&'static str, Vec<u8>, &'static str)>,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            listener.set_nonblocking(false).ok();
            for _ in 0..24 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    continue;
                }
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/");
                if path == "/__shutdown" {
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    );
                    break;
                }
                if let Some((_, body, content_type)) = routes.iter().find(|(p, _, _)| *p == path) {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        content_type,
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body);
                } else {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found",
                    );
                }
            }
        });
        (format!("http://127.0.0.1:{}", addr.port()), handle)
    }

    #[tokio::test]
    async fn test_n03_plan_resolves_non_mod_content_types() {
        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let instance = instances
            .create(launcher_types::CreateInstanceRequest {
                name: "N03 Test".into(),
                minecraft_version: "1.20.4".into(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Fabric,
                    version: Some("0.15.0".into()),
                },
            })
            .unwrap();

        let (base, handle) = spawn_mock_http_server(vec![
            (
                "/version/ver-shader",
                br#"{"id":"ver-shader","project_id":"proj-shader","name":"Complementary","version_number":"1.0","version_type":"release","game_versions":["1.20.4"],"loaders":["iris"],"downloads":10,"date_published":"2025-01-01T00:00:00Z","files":[{"hashes":{"sha1":"0000"},"url":"https://example.com/s.zip","filename":"comp.zip","primary":true,"size":10}],"dependencies":[]}"#.to_vec(),
                "application/json",
            ),
            (
                "/project/proj-shader",
                br#"{"id":"proj-shader","slug":"comp","title":"Complementary","description":"Shader","body":"","project_type":"shader","categories":[],"loaders":["iris"],"game_versions":["1.20.4"],"downloads":10,"icon_url":null,"updated":"2025-01-01T00:00:00Z"}"#.to_vec(),
                "application/json",
            ),
            (
                "/mods/101/files/202",
                br#"{"data":{"id":202,"modId":101,"displayName":"WorldMap","fileName":"MyWorld.zip","releaseType":1,"fileDate":"2025-01-01T00:00:00Z","downloadCount":5,"downloadUrl":"https://example.com/world.zip","hashes":[],"gameVersions":["1.20.4"],"dependencies":[]}}"#.to_vec(),
                "application/json",
            ),
            (
                "/mods/101",
                br#"{"data":{"id":101,"name":"Adventure World","slug":"adv-world","classId":17,"summary":"World","downloadCount":5,"dateModified":"2025-01-01T00:00:00Z","logo":null,"categories":[]}}"#.to_vec(),
                "application/json",
            ),
            (
                "/mods/101/description",
                br#"{"data":"<p>desc</p>"}"#.to_vec(),
                "application/json",
            ),
        ]);

        let http = launcher_net::HttpClient::new_loopback_test();
        let mr_plan = plan::build_install_plan_with_bases(
            &http,
            &instance,
            launcher_types::ContentProviderId::Modrinth,
            "ver-shader",
            "",
            &base,
            &base,
        )
        .await
        .unwrap();
        assert_eq!(mr_plan.target.content_type, ContentType::Shader);

        let cf_plan = plan::build_install_plan_with_bases(
            &http,
            &instance,
            launcher_types::ContentProviderId::Curseforge,
            "101:202",
            "dummy-key",
            &base,
            &base,
        )
        .await
        .unwrap();
        assert_eq!(cf_plan.target.content_type, ContentType::World);

        let _ = http.get(&format!("{}/__shutdown", base)).await;
        let _ = handle.join();
    }

    #[tokio::test]
    async fn test_n03_world_install_extracts_archive_to_saves() {
        use std::io::Write;

        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let http_dl = launcher_net::HttpClient::new_loopback_test();
        let downloader = launcher_download::DownloadManager::with_http_client(2, http_dl.clone());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let instance = instances
            .create(launcher_types::CreateInstanceRequest {
                name: "N03 World".into(),
                minecraft_version: "1.20.4".into(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Vanilla,
                    version: None,
                },
            })
            .unwrap();

        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            zw.start_file("AdventureWorld/level.dat", options).unwrap();
            zw.write_all(b"dummy-level-nbt").unwrap();
            zw.finish().unwrap();
        }
        let zip_bytes = cursor.into_inner();

        let (base, handle) =
            spawn_mock_http_server(vec![("/world.zip", zip_bytes, "application/zip")]);

        let plan = launcher_types::ContentInstallPlan {
            target: launcher_types::ContentPlanEntry {
                version: launcher_types::ContentVersion {
                    provider: launcher_types::ContentProviderId::Curseforge,
                    id: "101:202".into(),
                    project_id: "101".into(),
                    name: "AdventureWorld".into(),
                    version_number: "1.0".into(),
                    version_type: "release".into(),
                    game_versions: vec!["1.20.4".into()],
                    loaders: vec![],
                    downloads: 1,
                    published_at: "2025-01-01T00:00:00Z".into(),
                    file_name: "AdventureWorld.zip".into(),
                    download_url: Some(format!("{}/world.zip", base)),
                    sha1: None,
                },
                content_type: ContentType::World,
                downloadable: true,
            },
            dependencies: vec![],
            warnings: vec![],
        };

        let secrets = launcher_platform::SecretStore::new(paths.secrets_file.clone());
        let service = ContentService::with_http_client(
            paths.clone(),
            http_dl.clone(),
            downloader,
            instances,
            secrets,
        );
        let updated = service
            .install_plan(&instance.id, plan, None)
            .await
            .unwrap();

        let saves_dir = paths
            .instances_dir
            .join(&instance.id)
            .join("minecraft")
            .join("saves");
        assert!(
            saves_dir.join("AdventureWorld").join("level.dat").exists(),
            "World archive must be extracted into saves/AdventureWorld/level.dat"
        );
        assert!(
            !saves_dir.join("AdventureWorld.zip").exists(),
            "Raw world archive should not remain in saves/"
        );
        assert_eq!(
            updated.content[0].world_folder.as_deref(),
            Some("AdventureWorld")
        );

        let http = launcher_net::HttpClient::new_loopback_test();
        let _ = http.get(&format!("{}/__shutdown", base)).await;
        let _ = handle.join();
    }

    #[tokio::test]
    async fn test_n09_curseforge_non_2xx_status_checked_before_json() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://127.0.0.1:{}", addr.port());

        let handle = std::thread::spawn(move || {
            for _ in 0..3 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 2048];
                    let _ = stream.read(&mut buf);
                    let req = String::from_utf8_lossy(&buf);
                    if req.contains("GET /__shutdown") {
                        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
                        break;
                    } else if req.contains("/files/999") {
                        let body = b"{\"error\":\"rate limited\"}";
                        let hdr = format!(
                            "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(hdr.as_bytes());
                        let _ = stream.write_all(body);
                    } else {
                        let body = b"Forbidden";
                        let hdr = format!(
                            "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(hdr.as_bytes());
                        let _ = stream.write_all(body);
                    }
                }
            }
        });

        let http = launcher_net::HttpClient::new_loopback_test();
        let err_429 =
            curseforge::get_curseforge_file_with_base(&http, &base, "fake-key", "123", 999)
                .await
                .unwrap_err();
        assert_eq!(
            err_429.code,
            launcher_types::ErrorCode::RateLimited,
            "Expected RateLimited on HTTP 429, got {:?}: {}",
            err_429.code,
            err_429.message
        );

        let err_403 = curseforge::list_curseforge_versions_with_base(
            &http,
            &base,
            "bad-key",
            "123",
            &launcher_types::ContentFilter {
                minecraft_version: None,
                loader: None,
                featured: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            err_403.code,
            launcher_types::ErrorCode::ProviderNotConfigured,
            "Expected ProviderNotConfigured on HTTP 403, got {:?}: {}",
            err_403.code,
            err_403.message
        );

        let _ = http.get(&format!("{}/__shutdown", base)).await;
        let _ = handle.join();
    }

    #[tokio::test]
    async fn test_n10_curseforge_modpack_reports_blocked_or_failed_mods() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://127.0.0.1:{}", addr.port());

        let make_cf_pack = |files_json: &str| -> Vec<u8> {
            let mut buf = Vec::new();
            {
                let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
                let opts = zip::write::SimpleFileOptions::default();
                zw.start_file("manifest.json", opts).unwrap();
                let manifest = format!(
                    r#"{{"name":"CF Pack","minecraft":{{"version":"1.20.4","modLoaders":[]}},"files":{}}}"#,
                    files_json
                );
                zw.write_all(manifest.as_bytes()).unwrap();
                zw.finish().unwrap();
            }
            buf
        };
        let pack_err_zip = make_cf_pack(r#"[{"projectID":777,"fileID":888}]"#);
        let pack_blocked_zip = make_cf_pack(r#"[{"projectID":555,"fileID":666}]"#);

        let base_for_srv = base.clone();
        let handle = std::thread::spawn(move || loop {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let req = String::from_utf8_lossy(&buf);
                let send = |s: &mut std::net::TcpStream, status: &str, ct: &str, body: &[u8]| {
                    let hdr = format!(
                        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        status,
                        ct,
                        body.len()
                    );
                    let _ = s.write_all(hdr.as_bytes());
                    let _ = s.write_all(body);
                };

                if req.contains("GET /__shutdown") {
                    send(&mut stream, "200 OK", "text/plain", b"");
                    break;
                } else if req.contains("GET /v1/mods/100/files/101") {
                    let body = format!(
                        r#"{{"data":{{"id":101,"modId":100,"releaseType":1,"displayName":"PackErr","fileName":"pack-err.zip","downloadUrl":"{}/dl/pack-err.zip","hashes":[],"gameVersions":["1.20.4"],"dependencies":[]}}}}"#,
                        base_for_srv
                    );
                    send(&mut stream, "200 OK", "application/json", body.as_bytes());
                } else if req.contains("GET /v1/mods/200/files/201") {
                    let body = format!(
                        r#"{{"data":{{"id":201,"modId":200,"releaseType":1,"displayName":"PackBlocked","fileName":"pack-blocked.zip","downloadUrl":"{}/dl/pack-blocked.zip","hashes":[],"gameVersions":["1.20.4"],"dependencies":[]}}}}"#,
                        base_for_srv
                    );
                    send(&mut stream, "200 OK", "application/json", body.as_bytes());
                } else if req.contains("GET /dl/pack-err.zip") {
                    send(&mut stream, "200 OK", "application/zip", &pack_err_zip);
                } else if req.contains("GET /dl/pack-blocked.zip") {
                    send(&mut stream, "200 OK", "application/zip", &pack_blocked_zip);
                } else if req.contains("GET /v1/mods/555/files/666") {
                    let body = r#"{"data":{"id":666,"modId":555,"releaseType":1,"displayName":"BlockedMod","fileName":"blocked-mod.jar","downloadUrl":null,"hashes":[],"gameVersions":["1.20.4"],"dependencies":[]}}"#;
                    send(&mut stream, "200 OK", "application/json", body.as_bytes());
                } else {
                    send(&mut stream, "404 Not Found", "text/plain", b"Not Found");
                }
            }
        });

        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let http = launcher_net::HttpClient::new_loopback_test();
        let downloader = launcher_download::DownloadManager::with_http_client(2, http.clone());
        let api_base = format!("{}/v1", base);

        let res_err = modpack::install_modpack_with_base(
            &paths,
            &http,
            &downloader,
            &instances,
            launcher_types::ContentProviderId::Curseforge,
            "100:101",
            Some("PackErr"),
            "fake-key",
            &api_base,
            |_, _, _| {},
        )
        .await
        .expect_err("Errored mod in CurseForge modpack must not be silently skipped");
        assert!(
            res_err.message.contains("777") || res_err.message.contains("888"),
            "Error must report failing mod 777:888, got: {}",
            res_err.message
        );

        let res_blocked = modpack::install_modpack_with_base(
            &paths,
            &http,
            &downloader,
            &instances,
            launcher_types::ContentProviderId::Curseforge,
            "200:201",
            Some("PackBlocked"),
            "fake-key",
            &api_base,
            |_, _, _| {},
        )
        .await
        .expect_err(
            "Distribution-blocked mod must report error instead of silent skip/CDN failure",
        );
        assert_eq!(
            res_blocked.code,
            launcher_types::ErrorCode::DistributionBlocked,
            "Expected DistributionBlocked error code, got {:?}: {}",
            res_blocked.code,
            res_blocked.message
        );
        assert!(
            res_blocked.message.contains("blocked-mod.jar") || res_blocked.message.contains("555"),
            "Error must name the blocked mod, got: {}",
            res_blocked.message
        );

        let _ = http.get(&format!("{}/__shutdown", base)).await;
        let _ = handle.join();
    }

    #[tokio::test]
    async fn test_n12_content_and_modpack_cancellation() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let inst = instances
            .create(launcher_types::CreateInstanceRequest {
                name: "CancelPackTest".to_string(),
                minecraft_version: "1.20.1".to_string(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Fabric,
                    version: Some("0.15.0".to_string()),
                },
            })
            .unwrap();

        let http = launcher_net::HttpClient::new_loopback_test();
        let downloader = launcher_download::DownloadManager::with_http_client(2, http.clone());
        let service = ContentService::new(paths.clone(), downloader.clone(), instances.clone());

        let flag = Arc::new(AtomicBool::new(true));
        let err_content = service
            .install_with_cancel(
                &inst.id,
                launcher_types::ContentProviderId::Modrinth,
                "ver-1",
                None,
                Arc::clone(&flag),
            )
            .await
            .expect_err("Pre-cancelled content install must abort before hitting network");
        assert_eq!(err_content.code, launcher_types::ErrorCode::Cancelled);

        let err_modpack = modpack::install_modpack_with_cancel(
            &paths,
            &http,
            &downloader,
            &instances,
            launcher_types::ContentProviderId::Modrinth,
            "pack-ver-1",
            Some("CancelledPack"),
            "",
            |_, _, _| {},
            Arc::clone(&flag),
        )
        .await
        .expect_err("Pre-cancelled modpack install must abort immediately");
        assert_eq!(err_modpack.code, launcher_types::ErrorCode::Cancelled);
        assert!(
            !paths.cache_dir.join("modpack-staging").exists()
                || std::fs::read_dir(paths.cache_dir.join("modpack-staging"))
                    .unwrap()
                    .next()
                    .is_none(),
            "Modpack staging directory should remain clean on cancellation"
        );
    }

    #[test]
    fn test_ps08_ps18_tampered_instance_content_traversal_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let mut inst = instances
            .create(launcher_types::CreateInstanceRequest {
                name: "Tampered".into(),
                minecraft_version: "1.20.4".into(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Vanilla,
                    version: None,
                },
            })
            .unwrap();

        let mc_dir = paths.instances_dir.join(&inst.id).join("minecraft");
        let canary = mc_dir.join("canary.txt");
        std::fs::create_dir_all(mc_dir.join("mods")).unwrap();
        std::fs::write(&canary, b"do-not-delete").unwrap();

        inst.content.push(launcher_types::InstanceContentEntry {
            provider: launcher_types::ContentProviderId::Modrinth,
            project_id: "evil-mod".into(),
            version_id: "1".into(),
            content_type: ContentType::Mod,
            file_name: "../canary.txt".into(),
            sha1: String::new(),
            world_folder: None,
            disabled: Some(false),
        });
        instances.update(inst.clone()).unwrap();

        let service =
            ContentService::new(paths, launcher_download::DownloadManager::new(1), instances);
        assert!(service
            .toggle(
                &inst.id,
                launcher_types::ContentProviderId::Modrinth,
                "evil-mod",
                false
            )
            .is_err());
        assert!(service
            .remove(
                &inst.id,
                launcher_types::ContentProviderId::Modrinth,
                "evil-mod"
            )
            .is_err());
        assert!(canary.exists(), "Canary outside mods/ must not be deleted!");
    }

    #[test]
    fn test_os01_os14_no_builtin_cf_key_and_no_cdn_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let downloader = launcher_download::DownloadManager::new(1);
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let service = ContentService::new(paths.clone(), downloader, instances);

        if std::env::var_os("CURSEFORGE_API_KEY").is_none()
            && option_env!("CURSEFORGE_API_KEY").is_none()
        {
            assert!(
                service.get_curseforge_key().is_none(),
                "CurseForge key must default to None instead of a hardcoded key"
            );
            if std::env::var_os("CURSEFORGE_PROXY_URL").is_none()
                && option_env!("CURSEFORGE_PROXY_URL").is_none()
                && option_env!("MCLAUNCHER_DEFAULT_CF_PROXY").is_none()
            {
                assert!(service.get_curseforge_proxy_url().is_none());
                let status = service.integrations_status();
                assert!(!status.curseforge.configured);
                assert!(status.curseforge.source.is_none());
            }
        }

        launcher_storage::write_json_atomic(
            &paths.root.join("integrations.json"),
            &serde_json::json!({ "curseforgeProxyUrl": "https://cf-proxy.example.workers.dev" }),
        )
        .unwrap();
        assert_eq!(
            service.get_curseforge_proxy_url().as_deref(),
            Some("https://cf-proxy.example.workers.dev/v1")
        );
        if std::env::var_os("CURSEFORGE_API_KEY").is_none() {
            let proxy_status = service.integrations_status();
            assert!(proxy_status.curseforge.configured);
            assert_eq!(proxy_status.curseforge.source.as_deref(), Some("proxy"));
        }

        let blocked_file = curseforge::CfFile {
            id: 4567890,
            mod_id: 123456,
            display_name: "Blocked Mod".to_string(),
            file_name: "blocked mod.jar".to_string(),
            file_date: None,
            release_type: 1,
            download_count: Some(10),
            download_url: None,
            hashes: vec![],
            game_versions: vec!["1.20.4".to_string(), "Fabric".to_string()],
            dependencies: vec![],
        };
        let cv = curseforge::to_content_version(&blocked_file);
        assert!(
            cv.download_url.is_none(),
            "Must not fabricate CDN URL when download_url is None"
        );
    }

    #[tokio::test]
    async fn test_os01_curseforge_proxy_allows_empty_client_key() {
        let (base, handle) = spawn_mock_http_server(vec![(
            "/v1/mods/42/files/99",
            br#"{"data":{"id":99,"modId":42,"releaseType":1,"displayName":"ProxyMod","fileName":"proxy-mod.jar","downloadUrl":"https://cdn.example.invalid/proxy-mod.jar","hashes":[],"gameVersions":["1.20.4"],"dependencies":[]}}"#.to_vec(),
            "200 OK",
        )]);

        let http = launcher_net::HttpClient::new_loopback_test();
        let official_err = curseforge::get_curseforge_file(&http, "", "42", 99)
            .await
            .expect_err("Direct CurseForge API without key must return ProviderNotConfigured");
        assert_eq!(
            official_err.code,
            launcher_types::ErrorCode::ProviderNotConfigured
        );

        let proxy_base = format!("{}/v1", base);
        let file = curseforge::get_curseforge_file_with_base(&http, &proxy_base, "", "42", 99)
            .await
            .expect("CurseForge proxy mode must allow requests without client API key");
        assert_eq!(file.file_name, "proxy-mod.jar");

        let _ = http.get(&format!("{}/__shutdown", base)).await;
        let _ = handle.join();
    }

    #[tokio::test]
    async fn test_modpack_persists_icon_url_and_modpack_entry() {
        use std::io::Write;
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let mut mrpack_bytes = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut mrpack_bytes));
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("modrinth.index.json", opts).unwrap();
            let index = r#"{"name":"Fabulously Optimized","dependencies":{"minecraft":"1.21.4","fabric-loader":"0.16.10"},"files":[]}"#;
            zw.write_all(index.as_bytes()).unwrap();
            zw.finish().unwrap();
        }

        let (base, handle) = {
            use std::io::Read;
            use std::net::TcpListener;
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let base = format!("http://127.0.0.1:{}", addr.port());
            let base_srv = base.clone();
            let archive_body = mrpack_bytes.clone();
            let handle = std::thread::spawn(move || loop {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let req = String::from_utf8_lossy(&buf);
                let send = |s: &mut std::net::TcpStream, status: &str, ct: &str, body: &[u8]| {
                    let hdr = format!(
                        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        status, ct, body.len()
                    );
                    let _ = s.write_all(hdr.as_bytes());
                    let _ = s.write_all(body);
                };
                if req.contains("GET /__shutdown") {
                    send(&mut stream, "200 OK", "text/plain", b"");
                    break;
                } else if req.contains("GET /v2/version/ver-fo") {
                    let body = format!(
                        r#"{{"id":"ver-fo","project_id":"1KVo5zza","name":"FO 1.21.4","version_number":"6.0.0","version_type":"release","game_versions":["1.21.4"],"loaders":["fabric"],"downloads":1,"date_published":"2026-01-01T00:00:00Z","files":[{{"hashes":{{}},"url":"{}/dl/fo.mrpack","filename":"fo.mrpack","primary":true,"size":{}}}],"dependencies":[]}}"#,
                        base_srv,
                        archive_body.len()
                    );
                    send(&mut stream, "200 OK", "application/json", body.as_bytes());
                } else if req.contains("GET /v2/project/1KVo5zza") {
                    let body = format!(
                        r#"{{"id":"1KVo5zza","slug":"fabulously-optimized","title":"Fabulously Optimized","description":"Perf","body":"","project_type":"modpack","categories":[],"loaders":["fabric"],"game_versions":["1.21.4"],"downloads":1,"icon_url":"{}/icon.webp","updated":"2026-01-01T00:00:00Z"}}"#,
                        base_srv
                    );
                    send(&mut stream, "200 OK", "application/json", body.as_bytes());
                } else if req.contains("GET /dl/fo.mrpack") {
                    send(&mut stream, "200 OK", "application/zip", &archive_body);
                } else {
                    send(&mut stream, "404 Not Found", "text/plain", b"Not Found");
                }
            });
            (base, handle)
        };

        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let http = launcher_net::HttpClient::new_loopback_test();
        let downloader = launcher_download::DownloadManager::with_http_client(2, http.clone());
        let mr_base = format!("{}/v2", base);

        let installed = {
            let instances_obs = instances.clone();
            let paths_obs = paths.clone();
            let expected_obs = format!("{}/icon.webp", base);
            let saw_early_icon = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let saw_early_icon_cb = Arc::clone(&saw_early_icon);
            let result = modpack::install_modpack_with_bases_and_options(
                &paths,
                &http,
                &downloader,
                &instances,
                launcher_types::ContentProviderId::Modrinth,
                "ver-fo",
                Some("Fabulously Optimized"),
                "",
                &mr_base,
                &base,
                modpack::ModpackInstallOptions::default(),
                move |_, completed, total| {
                    if completed == 2 && total == 3 {
                        if let Ok(list) = instances_obs.list() {
                            if let Some(pending) = list.first() {
                                let marker = paths_obs
                                    .instances_dir
                                    .join(&pending.id)
                                    .join(launcher_storage::MODPACK_INSTALLING_MARKER);
                                if pending.icon_url.as_deref() == Some(expected_obs.as_str())
                                    && pending
                                        .content
                                        .first()
                                        .is_some_and(|e| e.content_type == ContentType::Modpack)
                                    && marker.exists()
                                {
                                    saw_early_icon_cb
                                        .store(true, std::sync::atomic::Ordering::SeqCst);
                                }
                            }
                        }
                    }
                },
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .expect("Modrinth modpack install with mock server should succeed");
            assert!(
                saw_early_icon.load(std::sync::atomic::Ordering::SeqCst),
                "iconUrl and Modpack content entry must be persisted when stage 2/3 begins"
            );
            result
        };

        let expected_icon = format!("{}/icon.webp", base);
        assert_eq!(installed.icon_url.as_deref(), Some(expected_icon.as_str()));
        assert_eq!(installed.content.len(), 1);
        assert_eq!(installed.content[0].content_type, ContentType::Modpack);
        assert_eq!(installed.content[0].project_id, "1KVo5zza");
        assert_eq!(installed.content[0].version_id, "ver-fo");
        assert_eq!(installed.content[0].file_name, "fo.mrpack");
        assert_eq!(installed.content[0].sha1, "");
        assert!(
            !paths
                .instances_dir
                .join(&installed.id)
                .join(launcher_storage::MODPACK_INSTALLING_MARKER)
                .exists(),
            "Installing marker must be removed on completion"
        );

        let stored = instances.get(&installed.id).unwrap();
        assert_eq!(stored.icon_url.as_deref(), Some(expected_icon.as_str()));
        assert_eq!(stored.content[0].content_type, ContentType::Modpack);

        let _ = http.get(&format!("{}/__shutdown", base)).await;
        let _ = handle.join();
    }

    #[tokio::test]
    async fn test_m5_m12_world_validation_and_check_updates_skip() {
        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let instances = launcher_storage::InstancesStore::new(paths.clone());
        let mut inst = instances
            .create(launcher_types::CreateInstanceRequest {
                name: "M5M12Test".to_string(),
                minecraft_version: "1.20.4".to_string(),
                loader: launcher_types::LoaderSpec {
                    loader_type: launcher_types::LoaderType::Fabric,
                    version: Some("0.15.0".to_string()),
                },
            })
            .unwrap();

        let saves_dir = paths
            .instances_dir
            .join(&inst.id)
            .join("minecraft")
            .join("saves");
        let existing_world = saves_dir.join("MyWorld");
        std::fs::create_dir_all(&existing_world).unwrap();
        std::fs::write(existing_world.join("level.dat"), b"level").unwrap();

        inst.content.push(launcher_types::InstanceContentEntry {
            provider: launcher_types::ContentProviderId::Modrinth,
            project_id: "1KVo5zza".to_string(),
            version_id: "ver-pack".to_string(),
            content_type: ContentType::Modpack,
            file_name: "pack.mrpack".to_string(),
            sha1: String::new(),
            world_folder: None,
            disabled: Some(false),
        });
        inst.content.push(launcher_types::InstanceContentEntry {
            provider: launcher_types::ContentProviderId::Modrinth,
            project_id: "mods/sodium.jar".to_string(),
            version_id: "mods/sodium.jar".to_string(),
            content_type: ContentType::Mod,
            file_name: "sodium.jar".to_string(),
            sha1: String::new(),
            world_folder: None,
            disabled: Some(false),
        });
        instances.update(inst.clone()).unwrap();

        let service =
            ContentService::new(paths, launcher_download::DownloadManager::new(1), instances);

        // M12: check_updates should skip modpack & path-based .mrpack entries cleanly
        let updates = service.check_updates(&inst.id).await.unwrap();
        assert!(updates.is_empty());

        // M5: validate world_id handling for saves
        let missing = launcher_security::safe_subpath(&saves_dir, "NoSuchWorld").unwrap();
        assert!(!missing.exists());
    }
}
