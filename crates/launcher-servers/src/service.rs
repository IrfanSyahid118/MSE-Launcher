use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use launcher_platform::DataPaths;
use launcher_security::validate_instance_id;
use launcher_types::{
    InstanceServerEntry, LauncherErrorPayload, ServerPingStatus, ServerResourcePackPolicy,
};

use crate::nbt::{read_nbt_buffer, write_nbt_buffer, NbtTag, NbtValue};
use crate::ping::query_minecraft_server_status;

type RawServerData = (HashMap<String, NbtTag>, Vec<HashMap<String, NbtTag>>);

#[derive(Clone)]
pub struct ServerService {
    paths: DataPaths,
}

impl ServerService {
    pub fn new(paths: DataPaths) -> Self {
        Self { paths }
    }

    fn servers_file(&self, instance_id: &str) -> Result<PathBuf, LauncherErrorPayload> {
        validate_instance_id(instance_id)?;
        Ok(self
            .paths
            .instances_dir
            .join(instance_id)
            .join("minecraft")
            .join("servers.dat"))
    }

    fn parse_server_compound(compound: &HashMap<String, NbtTag>) -> InstanceServerEntry {
        let name = match compound.get("name").map(|t| &t.value) {
            Some(NbtValue::String(s)) => s.clone(),
            _ => "Minecraft Server".to_string(),
        };

        let ip = match compound.get("ip").map(|t| &t.value) {
            Some(NbtValue::String(s)) => s.clone(),
            _ => "".to_string(),
        };

        let icon = match compound.get("icon").map(|t| &t.value) {
            Some(NbtValue::String(s)) => Some(s.clone()),
            _ => None,
        };

        let accept_textures = match compound.get("acceptTextures").map(|t| &t.value) {
            Some(NbtValue::Byte(b)) => Some(*b as u8),
            _ => None,
        };

        let hide_from_home = match compound.get("hidden").map(|t| &t.value) {
            Some(NbtValue::Byte(b)) => Some(*b != 0),
            _ => None,
        };

        let resource_pack_policy = match accept_textures {
            Some(1) => Some(ServerResourcePackPolicy::Enabled),
            Some(0) => Some(ServerResourcePackPolicy::Disabled),
            _ => Some(ServerResourcePackPolicy::Prompt),
        };

        InstanceServerEntry {
            name,
            ip,
            icon,
            accept_textures,
            resource_pack_policy,
            hide_from_home,
        }
    }

    fn create_server_compound(entry: &InstanceServerEntry) -> HashMap<String, NbtTag> {
        let mut map = HashMap::new();
        map.insert(
            "name".to_string(),
            NbtTag {
                tag_type: 8,
                value: NbtValue::String(entry.name.trim().to_string()),
            },
        );
        map.insert(
            "ip".to_string(),
            NbtTag {
                tag_type: 8,
                value: NbtValue::String(entry.ip.trim().to_string()),
            },
        );

        if let Some(ref icon) = entry.icon {
            map.insert(
                "icon".to_string(),
                NbtTag {
                    tag_type: 8,
                    value: NbtValue::String(icon.clone()),
                },
            );
        }

        let accept_val = match entry.resource_pack_policy {
            Some(ServerResourcePackPolicy::Enabled) => Some(1i8),
            Some(ServerResourcePackPolicy::Disabled) => Some(0i8),
            Some(ServerResourcePackPolicy::Prompt) => None,
            None => entry.accept_textures.map(|v| v as i8),
        };

        if let Some(val) = accept_val {
            map.insert(
                "acceptTextures".to_string(),
                NbtTag {
                    tag_type: 1,
                    value: NbtValue::Byte(val),
                },
            );
        }

        if let Some(hidden) = entry.hide_from_home {
            map.insert(
                "hidden".to_string(),
                NbtTag {
                    tag_type: 1,
                    value: NbtValue::Byte(if hidden { 1 } else { 0 }),
                },
            );
        }

        map
    }

    fn read_servers_raw(file_path: &Path) -> Result<RawServerData, LauncherErrorPayload> {
        match fs::symlink_metadata(file_path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target servers.dat tidak boleh berupa symlink.",
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((HashMap::new(), Vec::new()));
            }
            Err(e) => {
                return Err(LauncherErrorPayload::file_system(format!(
                    "Gagal memeriksa servers.dat: {}",
                    e
                )));
            }
        }

        let bytes = fs::read(file_path).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal membaca servers.dat: {}", e))
        })?;

        let root_res = read_nbt_buffer(&bytes);
        let root = match root_res {
            Ok((_name, r)) => r,
            Err(_) => {
                // Quarantine corrupt servers.dat before restore so backup is not overwritten.
                Self::quarantine_corrupt_file(file_path);

                let bak_path = file_path.with_extension("dat.bak");
                if bak_path.exists() {
                    if let Ok(bak_bytes) = fs::read(&bak_path) {
                        if let Ok((_name, bak_root)) = read_nbt_buffer(&bak_bytes) {
                            let _ = Self::write_bytes_atomic(file_path, &bak_bytes);
                            return Self::extract_server_compounds(bak_root);
                        }
                    }
                }

                return Ok((HashMap::new(), Vec::new()));
            }
        };

        Self::extract_server_compounds(root)
    }

    fn extract_server_compounds(
        root: HashMap<String, NbtTag>,
    ) -> Result<RawServerData, LauncherErrorPayload> {
        let mut server_compounds = Vec::new();
        if let Some(servers_tag) = root.get("servers") {
            if let NbtValue::List { items, .. } = &servers_tag.value {
                for item in items {
                    if let NbtValue::Compound(c) = item {
                        server_compounds.push(c.clone());
                    }
                }
            }
        }
        Ok((root, server_compounds))
    }

    fn save_servers_raw(
        file_path: &Path,
        mut root: HashMap<String, NbtTag>,
        server_compounds: Vec<HashMap<String, NbtTag>>,
    ) -> Result<(), LauncherErrorPayload> {
        if file_path.exists() {
            let bak_path = file_path.with_extension("dat.bak");
            let bak_is_symlink = fs::symlink_metadata(&bak_path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            if !bak_is_symlink {
                let _ = fs::copy(file_path, bak_path);
            }
        }

        let list_items = server_compounds
            .into_iter()
            .map(NbtValue::Compound)
            .collect();

        root.insert(
            "servers".to_string(),
            NbtTag {
                tag_type: 9,
                value: NbtValue::List {
                    item_type: 10,
                    items: list_items,
                },
            },
        );

        let out_bytes = write_nbt_buffer("", &root, false)?;

        Self::write_bytes_atomic(file_path, &out_bytes)?;

        Ok(())
    }

    fn quarantine_corrupt_file(file_path: &Path) {
        if !file_path.exists() {
            return;
        }
        let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let nonce = fastrand::u32(..);
        let corrupt_path = file_path.with_extension(format!("dat.corrupt-{}-{}", ts, nonce));
        let _ = fs::rename(file_path, corrupt_path);
    }

    fn write_bytes_atomic(file_path: &Path, bytes: &[u8]) -> Result<(), LauncherErrorPayload> {
        use std::io::Write;
        if let Some(parent) = file_path.parent() {
            launcher_security::ensure_secure_dir(parent, false)?;
        }
        if let Ok(meta) = fs::symlink_metadata(file_path) {
            if meta.file_type().is_symlink() {
                return Err(LauncherErrorPayload::invalid_input(
                    "Target servers.dat tidak boleh berupa symlink.",
                ));
            }
        }

        let tmp_path = file_path.with_extension(format!("dat.tmp-{}", fastrand::u64(..)));
        {
            let mut tmp_file = launcher_security::safe_create_new_file(&tmp_path, false)?;
            if let Err(e) = tmp_file.write_all(bytes) {
                let _ = fs::remove_file(&tmp_path);
                return Err(LauncherErrorPayload::file_system(format!(
                    "Gagal menulis temporary servers.dat: {}",
                    e
                )));
            }
        }

        fs::rename(&tmp_path, file_path).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            LauncherErrorPayload::file_system(format!("Gagal menyimpan atomik servers.dat: {}", e))
        })?;

        Ok(())
    }

    pub fn list(
        &self,
        instance_id: &str,
    ) -> Result<Vec<InstanceServerEntry>, LauncherErrorPayload> {
        let file_path = self.servers_file(instance_id)?;
        let (_root, server_compounds) = Self::read_servers_raw(&file_path)?;
        Ok(server_compounds
            .iter()
            .map(Self::parse_server_compound)
            .collect())
    }

    pub fn add(
        &self,
        instance_id: &str,
        entry: InstanceServerEntry,
    ) -> Result<Vec<InstanceServerEntry>, LauncherErrorPayload> {
        let target_ip = entry.ip.trim().to_string();
        if target_ip.is_empty() {
            return Err(LauncherErrorPayload::invalid_input(
                "Alamat server (IP) tidak boleh kosong.",
            ));
        }

        let file_path = self.servers_file(instance_id)?;
        let (root, mut server_compounds) = Self::read_servers_raw(&file_path)?;

        let new_compound = Self::create_server_compound(&entry);

        if let Some(pos) = server_compounds.iter().position(|c| {
            c.get("ip")
                .and_then(|t| match &t.value {
                    NbtValue::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .map(|s| s.eq_ignore_ascii_case(&target_ip))
                .unwrap_or(false)
        }) {
            server_compounds[pos] = new_compound;
        } else {
            server_compounds.push(new_compound);
        }

        Self::save_servers_raw(&file_path, root, server_compounds)?;
        self.list(instance_id)
    }

    pub fn remove(
        &self,
        instance_id: &str,
        ip: &str,
    ) -> Result<Vec<InstanceServerEntry>, LauncherErrorPayload> {
        let file_path = self.servers_file(instance_id)?;
        let (root, mut server_compounds) = Self::read_servers_raw(&file_path)?;

        let target_ip = ip.trim();
        server_compounds.retain(|c| {
            !c.get("ip")
                .and_then(|t| match &t.value {
                    NbtValue::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .map(|s| s.eq_ignore_ascii_case(target_ip))
                .unwrap_or(false)
        });

        Self::save_servers_raw(&file_path, root, server_compounds)?;
        self.list(instance_id)
    }

    pub fn edit(
        &self,
        instance_id: &str,
        old_ip: &str,
        updated: InstanceServerEntry,
    ) -> Result<Vec<InstanceServerEntry>, LauncherErrorPayload> {
        if updated.ip.trim().is_empty() {
            return Err(LauncherErrorPayload::invalid_input(
                "Alamat server (IP) tidak boleh kosong.",
            ));
        }
        let file_path = self.servers_file(instance_id)?;
        let (root, mut server_compounds) = Self::read_servers_raw(&file_path)?;

        let target_old = old_ip.trim();
        let new_compound = Self::create_server_compound(&updated);

        if let Some(pos) = server_compounds.iter().position(|c| {
            c.get("ip")
                .and_then(|t| match &t.value {
                    NbtValue::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .map(|s| s.eq_ignore_ascii_case(target_old))
                .unwrap_or(false)
        }) {
            server_compounds[pos] = new_compound;
        } else {
            server_compounds.push(new_compound);
        }

        Self::save_servers_raw(&file_path, root, server_compounds)?;
        self.list(instance_id)
    }

    pub async fn ping(
        &self,
        ip: &str,
        _instance_id: Option<&str>,
    ) -> Result<ServerPingStatus, LauncherErrorPayload> {
        Ok(query_minecraft_server_status(ip, Duration::from_millis(4500)).await)
    }
}
