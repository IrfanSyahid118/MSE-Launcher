pub mod nbt;
pub mod ping;
pub mod service;

pub use nbt::*;
pub use ping::*;
pub use service::*;

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_types::ServerResourcePackPolicy;
    use tempfile::tempdir;

    #[test]
    fn test_nbt_roundtrip() {
        let mut root = std::collections::HashMap::new();
        root.insert(
            "name".to_string(),
            NbtTag {
                tag_type: 8,
                value: NbtValue::String("Test Server".to_string()),
            },
        );
        root.insert(
            "ip".to_string(),
            NbtTag {
                tag_type: 8,
                value: NbtValue::String("play.example.com".to_string()),
            },
        );
        root.insert(
            "acceptTextures".to_string(),
            NbtTag {
                tag_type: 1,
                value: NbtValue::Byte(1),
            },
        );

        let uncompressed = write_nbt_buffer("", &root, false).expect("write failed");
        let (_name, parsed) = read_nbt_buffer(&uncompressed).expect("read failed");
        assert_eq!(
            parsed.get("name").map(|t| &t.value),
            Some(&NbtValue::String("Test Server".to_string()))
        );
        assert_eq!(
            parsed.get("ip").map(|t| &t.value),
            Some(&NbtValue::String("play.example.com".to_string()))
        );
    }

    #[test]
    fn test_server_service_crud() {
        let tmp = tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(tmp.path());
        let service = ServerService::new(paths);

        let instance_id = "test-instance";

        // List initial
        let initial = service.list(instance_id).expect("list failed");
        assert!(initial.is_empty());

        // Add
        let server1 = launcher_types::InstanceServerEntry {
            name: "Hypixel".to_string(),
            ip: "mc.hypixel.net".to_string(),
            icon: None,
            accept_textures: Some(1),
            resource_pack_policy: Some(ServerResourcePackPolicy::Enabled),
            hide_from_home: Some(false),
        };
        let after_add = service
            .add(instance_id, server1.clone())
            .expect("add failed");
        assert_eq!(after_add.len(), 1);
        assert_eq!(after_add[0].name, "Hypixel");
        assert_eq!(after_add[0].ip, "mc.hypixel.net");

        // Edit
        let server1_edit = launcher_types::InstanceServerEntry {
            name: "Hypixel Network".to_string(),
            ip: "mc.hypixel.net:25565".to_string(),
            icon: None,
            accept_textures: Some(1),
            resource_pack_policy: Some(ServerResourcePackPolicy::Enabled),
            hide_from_home: Some(false),
        };
        let after_edit = service
            .edit(instance_id, "mc.hypixel.net", server1_edit)
            .expect("edit failed");
        assert_eq!(after_edit.len(), 1);
        assert_eq!(after_edit[0].name, "Hypixel Network");
        assert_eq!(after_edit[0].ip, "mc.hypixel.net:25565");

        // Remove
        let after_remove = service
            .remove(instance_id, "mc.hypixel.net:25565")
            .expect("remove failed");
        assert!(after_remove.is_empty());
    }

    #[test]
    fn test_parse_host_port() {
        assert_eq!(
            parse_host_port("localhost"),
            ("localhost".to_string(), 25565)
        );
        assert_eq!(
            parse_host_port("play.example.com:25570"),
            ("play.example.com".to_string(), 25570)
        );
        assert_eq!(parse_host_port("[::1]:25565"), ("::1".to_string(), 25565));
    }

    #[test]
    fn test_decode_varint_malformed_overflow() {
        // 5 consecutive bytes with continuation bit set followed by a 6th byte
        // In vulnerable code, this shifts val by 35 bits, causing an overflow panic!
        let malformed = [0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        let result = decode_varint(&malformed, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_nbt_string_length_clamp_above_u16_max_bug19() {
        let mut root = std::collections::HashMap::new();
        let huge_string = "A".repeat(70_000);
        root.insert(
            "huge".to_string(),
            NbtTag {
                tag_type: 8,
                value: NbtValue::String(huge_string),
            },
        );

        let uncompressed = write_nbt_buffer("", &root, false).expect("write failed");
        let (_name, parsed) = read_nbt_buffer(&uncompressed).expect("read failed");
        if let Some(NbtValue::String(s)) = parsed.get("huge").map(|t| &t.value) {
            assert_eq!(s.len(), u16::MAX as usize);
        } else {
            panic!("Expected huge string tag");
        }
    }

    #[tokio::test]
    async fn test_ping_hostile_server_packet_bounds_bug02() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        // Hostile server task: returns huge packet_len varint (10MB)
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                // Send huge varint (10_000_000)
                let huge_varint = encode_varint(10_000_000);
                use tokio::io::AsyncWriteExt;
                let _ = socket.write_all(&huge_varint).await;
            }
        });

        let res = query_minecraft_server_status(
            &format!("127.0.0.1:{}", port),
            std::time::Duration::from_millis(500),
        )
        .await;
        assert!(
            !res.online,
            "Hostile server with oversized packet_len must be rejected as offline"
        );
    }

    #[test]
    fn test_n22_corrupt_servers_dat_restored_and_quarantined_before_next_save() {
        let tmp = tempdir().unwrap();
        let paths = launcher_platform::DataPaths::new(tmp.path());
        let service = ServerService::new(paths.clone());
        let instance_id = "n22-inst";

        let s1 = launcher_types::InstanceServerEntry {
            name: "One".into(),
            ip: "one.example.com".into(),
            icon: None,
            accept_textures: None,
            resource_pack_policy: None,
            hide_from_home: None,
        };
        let s2 = launcher_types::InstanceServerEntry {
            name: "Two".into(),
            ip: "two.example.com".into(),
            icon: None,
            accept_textures: None,
            resource_pack_policy: None,
            hide_from_home: None,
        };
        let s3 = launcher_types::InstanceServerEntry {
            name: "Three".into(),
            ip: "three.example.com".into(),
            icon: None,
            accept_textures: None,
            resource_pack_policy: None,
            hide_from_home: None,
        };

        service.add(instance_id, s1).unwrap();
        service.add(instance_id, s2).unwrap();

        let mc_dir = paths.instances_dir.join(instance_id).join("minecraft");
        let dat_path = mc_dir.join("servers.dat");
        let bak_path = mc_dir.join("servers.dat.bak");
        assert!(dat_path.exists());
        assert!(bak_path.exists());

        std::fs::write(&dat_path, b"corrupt-nbt-bytes").unwrap();

        let restored = service.list(instance_id).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].ip, "one.example.com");

        let dat_bytes_after_list = std::fs::read(&dat_path).unwrap();
        assert!(
            read_nbt_buffer(&dat_bytes_after_list).is_ok(),
            "servers.dat must be restored from valid .bak instead of remaining corrupt"
        );

        let has_quarantine = std::fs::read_dir(&mc_dir)
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("servers.dat.corrupt-")
            });
        assert!(has_quarantine, "corrupt servers.dat must be quarantined");

        service.add(instance_id, s3).unwrap();
        let bak_bytes_after_save = std::fs::read(&bak_path).unwrap();
        assert!(
            read_nbt_buffer(&bak_bytes_after_save).is_ok(),
            "subsequent save must not overwrite servers.dat.bak with corrupt bytes"
        );
    }
}
