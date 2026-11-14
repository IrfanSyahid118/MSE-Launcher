pub mod fabric;
pub mod forge;
pub mod merge;
pub mod models;
pub mod neoforge;
pub mod quilt;
pub mod service;

pub use merge::*;
pub use models::*;
pub use service::LoaderService;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::parse_forge_metadata;
    use crate::neoforge::neoforge_to_game_version;
    use launcher_version::{MojangLibrary, MojangVersionDetail};
    use std::cmp::Ordering;

    #[test]
    fn test_compare_loader_versions() {
        assert_eq!(
            models::compare_loader_versions("0.16.9", "0.15.11"),
            Ordering::Less
        );
        assert_eq!(
            models::compare_loader_versions("0.15.11", "0.16.9"),
            Ordering::Greater
        );
        assert_eq!(
            models::compare_loader_versions("1.20.1", "1.20.1"),
            Ordering::Equal
        );
        assert_eq!(
            models::compare_loader_versions("1.20.1", "1.20.1-rc1"),
            Ordering::Less
        );
        assert_eq!(
            models::compare_loader_versions("1.20.1-rc1", "1.20.1"),
            Ordering::Greater
        );
        assert_eq!(
            models::compare_loader_versions("0.25.0-beta.2", "0.25.0-beta.1"),
            Ordering::Less
        );
        assert_eq!(
            models::compare_loader_versions("1.20.1", "1.20"),
            Ordering::Less
        );
    }

    #[test]
    fn test_neoforge_version_mapping() {
        assert_eq!(
            neoforge_to_game_version("20.4.80-beta"),
            Some("1.20.4".into())
        );
        assert_eq!(neoforge_to_game_version("21.0.12"), Some("1.21".into()));
        assert_eq!(neoforge_to_game_version("21.1.45"), Some("1.21.1".into()));
    }

    #[test]
    fn test_parse_forge_metadata() {
        let xml = r#"
            <metadata>
                <versioning>
                    <versions>
                        <version>1.20.1-47.2.0</version>
                        <version>1.20.1-47.2.20</version>
                        <version>1.19.2-43.2.0</version>
                    </versions>
                </versioning>
            </metadata>
        "#;
        let index = parse_forge_metadata(xml);
        assert!(index.game_versions.contains(&"1.20.1".to_string()));
        assert!(index.game_versions.contains(&"1.19.2".to_string()));
        let v_1201 = index.versions_by_game.get("1.20.1").unwrap();
        assert_eq!(v_1201.len(), 2);
        assert_eq!(v_1201[0], "47.2.20"); // descending order
    }

    #[test]
    fn test_merge_version_detail() {
        let parent = MojangVersionDetail {
            id: "1.20.1".into(),
            main_class: Some("net.minecraft.client.main.Main".into()),
            libraries: vec![
                MojangLibrary {
                    name: "org.ow2.asm:asm:9.3".into(),
                    ..Default::default()
                },
                MojangLibrary {
                    name: "com.google.guava:guava:31.1-jre".into(),
                    ..Default::default()
                },
            ],
            arguments: Some(serde_json::json!({
                "game": ["--username", "${auth_player_name}"],
                "jvm": ["-Djava.library.path=${natives_directory}"]
            })),
            ..Default::default()
        };

        let child = LoaderProfile {
            id: "fabric-loader-0.15.11-1.20.1".into(),
            inherits_from: "1.20.1".into(),
            main_class: Some("net.fabricmc.loader.impl.launch.knot.KnotClient".into()),
            profile_type: Some("release".into()),
            release_time: None,
            libraries: vec![
                MojangLibrary {
                    name: "org.ow2.asm:asm:9.6".into(), // overrides asm 9.3
                    ..Default::default()
                },
                MojangLibrary {
                    name: "net.fabricmc:fabric-loader:0.15.11".into(),
                    ..Default::default()
                },
            ],
            arguments: Some(serde_json::json!({
                "game": [],
                "jvm": ["-Dfabric.development=false"]
            })),
            minecraft_arguments: None,
        };

        let merged = merge_version_detail(&parent, &child).expect("Merge failed");
        assert_eq!(merged.id, "fabric-loader-0.15.11-1.20.1");
        assert_eq!(merged.inherits_from.as_deref(), Some("1.20.1"));
        assert_eq!(
            merged.main_class.as_deref(),
            Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        );
        assert_eq!(merged.libraries.len(), 3);
        // asm should now be 9.6
        let asm = merged
            .libraries
            .iter()
            .find(|l| l.name.starts_with("org.ow2.asm:asm:"))
            .unwrap();
        assert_eq!(asm.name, "org.ow2.asm:asm:9.6");
    }

    #[tokio::test]
    async fn test_n04_legacy_install_profile_version_info_extracted() {
        use std::io::Write;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let versions = launcher_version::VersionService::new(paths.clone());
        let loader_service = LoaderService::new(paths.clone(), versions);

        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            zw.start_file("install_profile.json", options).unwrap();
            zw.write_all(
                br#"{
                    "install": {
                        "minecraft": "1.8.9",
                        "path": "net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9",
                        "filePath": "forge-1.8.9-11.15.1.2318-1.8.9-universal.jar"
                    },
                    "versionInfo": {
                        "id": "1.8.9-forge1.8.9-11.15.1.2318-1.8.9",
                        "mainClass": "net.minecraft.launchwrapper.Launch",
                        "minecraftArguments": "--username ${auth_player_name} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
                        "libraries": [
                            {"name": "net.minecraftforge:forge:1.8.9-11.15.1.2318-1.8.9"}
                        ]
                    }
                }"#,
            )
            .unwrap();
            zw.start_file("forge-1.8.9-11.15.1.2318-1.8.9-universal.jar", options)
                .unwrap();
            zw.write_all(b"LEGACY_UNIVERSAL_JAR_BYTES").unwrap();
            zw.finish().unwrap();
        }
        let jar_bytes = cursor.into_inner();

        let runner_called = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&runner_called);
        let runner: crate::service::InstallerRunner<'_> = Box::new(move |_, _| {
            flag.store(true, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        });

        let profile = loader_service
            .prepare_installer_from_bytes(
                &jar_bytes,
                "1.8.9",
                "forge-1.8.9-11.15.1.2318-1.8.9",
                None,
                runner,
            )
            .await
            .unwrap();

        assert!(
            !runner_called.load(Ordering::SeqCst),
            "Legacy versionInfo must not spawn external --installClient"
        );
        assert_eq!(profile.inherits_from, "1.8.9");
        assert_eq!(
            profile.main_class.as_deref(),
            Some("net.minecraft.launchwrapper.Launch")
        );
        let extracted_lib = paths.libraries_dir.join(
            "net/minecraftforge/forge/1.8.9-11.15.1.2318-1.8.9/forge-1.8.9-11.15.1.2318-1.8.9.jar",
        );
        assert_eq!(
            std::fs::read(&extracted_lib).unwrap(),
            b"LEGACY_UNIVERSAL_JAR_BYTES"
        );
    }

    #[tokio::test]
    async fn test_n04_modern_install_profile_processors_runs_install_client() {
        use std::io::Write;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let temp = tempfile::tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(temp.path());
        let versions = launcher_version::VersionService::new(paths.clone());
        let loader_service = LoaderService::new(paths.clone(), versions);

        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            zw.start_file("version.json", options).unwrap();
            zw.write_all(
                br#"{"id":"1.20.4-forge-49.0.3","inheritsFrom":"1.20.4","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher","libraries":[]}"#,
            )
            .unwrap();
            zw.start_file("install_profile.json", options).unwrap();
            zw.write_all(
                br#"{"spec":1,"version":"1.20.4-forge-49.0.3","minecraft":"1.20.4","libraries":[{"name":"net.minecraftforge:installertools:1.4.1"}],"processors":[{"sides":["client"],"jar":"net.minecraftforge:installertools:1.4.1","args":["--task","MCP_DATA"]}]}"#,
            )
            .unwrap();
            zw.finish().unwrap();
        }
        let jar_bytes = cursor.into_inner();

        let runner_called = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&runner_called);
        let versions_dir = paths.versions_dir.clone();
        let runner: crate::service::InstallerRunner<'_> = Box::new(move |jar_path, shared_dir| {
            let jar_exists = jar_path.exists();
            let profiles_exists = shared_dir.join("launcher_profiles.json").exists();
            let versions_dir = versions_dir.clone();
            let flag = Arc::clone(&flag);
            Box::pin(async move {
                assert!(jar_exists);
                assert!(profiles_exists);
                flag.store(true, Ordering::SeqCst);
                let gen_dir = versions_dir.join("1.20.4-forge-49.0.3");
                std::fs::create_dir_all(&gen_dir).unwrap();
                std::fs::write(
                    gen_dir.join("1.20.4-forge-49.0.3.json"),
                    br#"{"id":"1.20.4-forge-49.0.3","inheritsFrom":"1.20.4","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher","libraries":[{"name":"net.minecraftforge:forge:1.20.4-49.0.3:client"}]}"#,
                )
                .unwrap();
                Ok(())
            })
        });

        let profile = loader_service
            .prepare_installer_from_bytes(&jar_bytes, "1.20.4", "forge-1.20.4-49.0.3", None, runner)
            .await
            .unwrap();

        assert!(
            runner_called.load(Ordering::SeqCst),
            "Modern installer with processors must invoke --installClient runner"
        );
        assert_eq!(profile.id, "forge-1.20.4-49.0.3");
        assert!(profile
            .libraries
            .iter()
            .any(|l| l.name == "net.minecraftforge:forge:1.20.4-49.0.3:client"));
        assert!(paths
            .versions_dir
            .join("forge-1.20.4-49.0.3")
            .join(".installer_done")
            .exists());
    }
}
