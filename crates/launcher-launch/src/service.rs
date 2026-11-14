use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use launcher_download::DownloadManager;
use launcher_java::JavaService;
use launcher_loader::LoaderService;
use launcher_platform::{DataPaths, SecretStore};
use launcher_storage::{AccountsStore, InstancesStore, SettingsStore};
use launcher_types::{
    AccountType, Instance, InstanceJavaMode, JavaInstallation, JvmPreset, LauncherErrorPayload,
    Settings,
};
use launcher_version::VersionService;

use crate::args::{create_argfile, expand_arguments_json, get_preset_jvm_args, should_use_argfile};
use crate::classpath::assemble_classpath;
use crate::natives::extract_natives;
use crate::process::{spawn_game_process, LogLine, RunningGame};

#[cfg(windows)]
const CLASSPATH_SEPARATOR: &str = ";";
#[cfg(not(windows))]
const CLASSPATH_SEPARATOR: &str = ":";

#[derive(Clone)]
enum LaunchSlot {
    Launching,
    Running(Arc<RunningGame>),
}

struct LaunchSlotGuard {
    running: Arc<RwLock<HashMap<String, LaunchSlot>>>,
    instance_id: String,
    armed: bool,
}

impl LaunchSlotGuard {
    fn new(running: Arc<RwLock<HashMap<String, LaunchSlot>>>, instance_id: String) -> Self {
        Self {
            running,
            instance_id,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for LaunchSlotGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let id = self.instance_id.clone();
        if let Ok(mut guard) = self.running.try_write() {
            if matches!(guard.get(&id), Some(LaunchSlot::Launching)) {
                guard.remove(&id);
            }
        } else if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let map = Arc::clone(&self.running);
            handle.spawn(async move {
                let mut guard = map.write().await;
                if matches!(guard.get(&id), Some(LaunchSlot::Launching)) {
                    guard.remove(&id);
                }
            });
        }
    }
}

#[derive(Clone)]
pub struct LaunchService {
    paths: DataPaths,
    versions: VersionService,
    loaders: LoaderService,
    downloader: DownloadManager,
    java: JavaService,
    settings: SettingsStore,
    instances: InstancesStore,
    accounts: AccountsStore,
    secrets: SecretStore,
    running: Arc<RwLock<HashMap<String, LaunchSlot>>>,
}

impl LaunchService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        paths: DataPaths,
        versions: VersionService,
        loaders: LoaderService,
        downloader: DownloadManager,
        java: JavaService,
        settings: SettingsStore,
        instances: InstancesStore,
        accounts: AccountsStore,
        secrets: SecretStore,
    ) -> Self {
        Self {
            paths,
            versions,
            loaders,
            downloader,
            java,
            settings,
            instances,
            accounts,
            secrets,
            running: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn is_running(&self, instance_id: &str) -> bool {
        self.running.read().await.contains_key(instance_id)
    }

    pub async fn kill(&self, instance_id: &str) -> Result<(), LauncherErrorPayload> {
        let slot = self.running.read().await.get(instance_id).cloned();
        match slot {
            Some(LaunchSlot::Running(game)) => {
                game.kill().await?;
                self.running.write().await.remove(instance_id);
                Ok(())
            }
            Some(LaunchSlot::Launching) => Err(LauncherErrorPayload::invalid_input(format!(
                "Instance '{}' sedang dalam proses peluncuran.",
                instance_id
            ))),
            None => Err(LauncherErrorPayload::not_found(format!(
                "Instance '{}' tidak sedang berjalan.",
                instance_id
            ))),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn launch<F1, F2, F3>(
        &self,
        instance_id: &str,
        version_id: &str,
        server_address: Option<&str>,
        on_log: F1,
        on_exit: F2,
        on_progress: F3,
        cancelled: Arc<AtomicBool>,
    ) -> Result<u32, LauncherErrorPayload>
    where
        F1: Fn(Vec<LogLine>) + Send + Sync + 'static,
        F2: Fn(Option<i32>) + Send + Sync + 'static,
        F3: Fn(String, u32, u32) + Send + Sync + 'static,
    {
        launcher_security::validate_instance_id(instance_id)?;
        launcher_security::validate_safe_single_component(version_id)?;

        {
            let mut slots = self.running.write().await;
            if slots.contains_key(instance_id) {
                return Err(LauncherErrorPayload::invalid_input(
                    "Game untuk instance ini sudah berjalan.",
                ));
            }
            slots.insert(instance_id.to_string(), LaunchSlot::Launching);
        }

        let mut slot_guard =
            LaunchSlotGuard::new(Arc::clone(&self.running), instance_id.to_string());
        let res = self
            .launch_inner(
                instance_id,
                version_id,
                server_address,
                on_log,
                on_exit,
                on_progress,
                cancelled,
                &mut slot_guard,
            )
            .await;

        if res.is_err() {
            slot_guard.disarm();
            let mut slots = self.running.write().await;
            if matches!(slots.get(instance_id), Some(LaunchSlot::Launching)) {
                slots.remove(instance_id);
            }
        }

        res
    }

    #[allow(clippy::too_many_arguments)]
    async fn launch_inner<F1, F2, F3>(
        &self,
        instance_id: &str,
        version_id: &str,
        server_address: Option<&str>,
        on_log: F1,
        on_exit: F2,
        on_progress: F3,
        cancelled: Arc<AtomicBool>,
        slot_guard: &mut LaunchSlotGuard,
    ) -> Result<u32, LauncherErrorPayload>
    where
        F1: Fn(Vec<LogLine>) + Send + Sync + 'static,
        F2: Fn(Option<i32>) + Send + Sync + 'static,
        F3: Fn(String, u32, u32) + Send + Sync + 'static,
    {
        let instance = self.instances.get(instance_id)?;
        let settings = self.settings.get();

        // 1. Dapatkan akun aktif
        let active_account_id = settings.active_account_id.as_deref().ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Pilih atau buat akun sebelum bermain.")
        })?;

        let mut account = self
            .accounts
            .get(active_account_id)
            .ok_or_else(|| LauncherErrorPayload::not_found("Akun aktif tidak ditemukan."))?;

        // SEC-03: Ambil token autentikasi terenkripsi dari SecretStore untuk akun Microsoft
        if account.account_type == AccountType::Microsoft {
            if account.tokens.is_none() {
                let secret_key = format!("tokens:{}", account.id);
                if let Some(secret_data) = self.secrets.get(&secret_key).await {
                    if let Ok(tokens) =
                        serde_json::from_str::<launcher_types::AccountTokens>(&secret_data)
                    {
                        account.tokens = Some(tokens);
                    } else {
                        account.tokens = Some(launcher_types::AccountTokens {
                            access_token: secret_data.clone(),
                            refresh_token: secret_data,
                        });
                    }
                }
            }

            // Periksa kedaluwarsa sesi (refresh otomatis jika < 5 menit sebelum expired)
            let needs_refresh = if let Some(ref exp_str) = account.expires_at {
                if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(exp_str) {
                    exp.with_timezone(&chrono::Utc)
                        <= chrono::Utc::now() + chrono::Duration::minutes(5)
                } else {
                    true
                }
            } else {
                false
            };

            if needs_refresh {
                let auth_service = launcher_auth::MicrosoftAuthService::new(
                    self.paths.clone(),
                    self.accounts.clone(),
                    self.secrets.clone(),
                );
                account = auth_service.refresh_session(&account).await?;
            }
        }

        on_progress("Memeriksa kelengkapan versi Minecraft...".to_string(), 1, 5);

        // 2. Dapatkan version detail (mendukung Vanilla, Fabric, Quilt, Forge, NeoForge)
        let loader_java_path = if matches!(
            instance.loader.loader_type,
            launcher_types::LoaderType::Forge | launcher_types::LoaderType::Neoforge
        ) {
            let parent_detail = self
                .versions
                .get_version_detail(&instance.minecraft_version)
                .await?;
            let req_major = parent_detail
                .java_version
                .as_ref()
                .map(|j| j.major_version)
                .unwrap_or(17);
            let req_comp = parent_detail
                .java_version
                .as_ref()
                .and_then(|j| j.component.as_deref());
            let resolved = self
                .resolve_java_for_launch(
                    &instance,
                    &settings,
                    req_major,
                    req_comp,
                    false,
                    &on_progress,
                    Arc::clone(&cancelled),
                )
                .await?;
            Some(std::path::PathBuf::from(resolved.path))
        } else {
            None
        };

        let detail = self
            .loaders
            .ensure_profile_with_java(
                instance.loader.loader_type,
                &instance.minecraft_version,
                instance.loader.version.as_deref(),
                loader_java_path.as_deref(),
            )
            .await?;

        // 3. Pastikan semua berkas game dan library sudah terunduh
        on_progress("Mengunduh library dan file game...".to_string(), 2, 5);
        let version_tasks = self.versions.build_version_download_tasks(&detail)?;
        self.downloader
            .download_all(version_tasks, |_| {}, Arc::clone(&cancelled))
            .await?;

        if cancelled.load(Ordering::Relaxed) {
            return Err(LauncherErrorPayload::internal("Peluncuran dibatalkan."));
        }

        // 4. Unduh aset game jika diperlukan
        on_progress("Memeriksa aset game...".to_string(), 3, 5);
        let asset_tasks = self.versions.fetch_asset_tasks(&detail).await?;
        self.downloader
            .download_all(asset_tasks, |_| {}, Arc::clone(&cancelled))
            .await?;

        if cancelled.load(Ordering::Relaxed) {
            return Err(LauncherErrorPayload::internal("Peluncuran dibatalkan."));
        }

        // 5. Bersihkan natives/argfiles lama lalu ekstrak natives ke direktori unik per peluncuran
        on_progress("Menyiapkan library native...".to_string(), 4, 5);
        let instance_dir = launcher_security::safe_subpath(&self.paths.instances_dir, instance_id)?;
        launcher_security::ensure_descendants_no_symlink(&self.paths.instances_dir, &instance_dir)?;

        if let Ok(entries) = std::fs::read_dir(&instance_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name == "natives" || name.starts_with("natives-") {
                        let _ = launcher_security::safe_remove_dir_all_within(&instance_dir, &p);
                    } else if name.starts_with("args-") && name.ends_with(".txt") {
                        let _ = launcher_security::safe_remove_file_within(&instance_dir, &p);
                    }
                }
            }
        }

        let natives_dir = extract_natives(&detail, &self.paths, &instance_dir)?;
        let mut natives_guard = NativesDirGuard::new(instance_dir.clone(), natives_dir.clone());

        // 6. Resolusi Java executable (FN-08: Otomatis unduh managed Mojang JRE jika belum ada)
        let required_major = detail
            .java_version
            .as_ref()
            .map(|j| j.major_version)
            .unwrap_or(8);
        let java_component = detail
            .java_version
            .as_ref()
            .and_then(|j| j.component.as_deref());

        let java_inst = self
            .resolve_java_for_launch(
                &instance,
                &settings,
                required_major,
                java_component,
                true,
                &on_progress,
                Arc::clone(&cancelled),
            )
            .await?;

        // 7. Bentuk Classpath
        let classpath = assemble_classpath(&detail, &self.paths);
        let mut cp_parts = Vec::with_capacity(classpath.len());
        for p in &classpath {
            let s = p.to_str().ok_or_else(|| {
                LauncherErrorPayload::invalid_input(
                    "Path classpath mengandung karakter bukan UTF-8.",
                )
            })?;
            cp_parts.push(s);
        }
        let classpath_str = cp_parts.join(CLASSPATH_SEPARATOR);

        let game_dir = launcher_security::safe_subpath(&instance_dir, "minecraft")?;
        launcher_security::ensure_dir_within(&instance_dir, &game_dir, false)?;

        // Offline skin injection: Resource Pack otomatis (Vanilla & Modded) + CustomSkinLoader (Modded)
        self.prepare_skin_artifacts(&instance, &account, &game_dir, &cancelled)
            .await?;

        let game_assets_dir = self
            .versions
            .prepare_game_assets(&detail, &game_dir)
            .await?;

        // 8. Bentuk Variabel Substitusi
        let mut vars = HashMap::new();
        vars.insert("auth_player_name", account.username.as_str());
        vars.insert("version_name", version_id);
        vars.insert("version_type", "release");
        let game_dir_str = game_dir.to_str().ok_or_else(|| {
            LauncherErrorPayload::invalid_input(
                "Path direktori game mengandung karakter bukan UTF-8.",
            )
        })?;
        vars.insert("game_directory", game_dir_str);
        let assets_root_str = self.paths.assets_dir.to_str().ok_or_else(|| {
            LauncherErrorPayload::invalid_input(
                "Path direktori aset mengandung karakter bukan UTF-8.",
            )
        })?;
        let game_assets_str = game_assets_dir.to_str().ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Path game assets mengandung karakter bukan UTF-8.")
        })?;
        vars.insert("assets_root", assets_root_str);
        vars.insert("game_assets", game_assets_str);
        let assets_index_str = detail
            .assets
            .clone()
            .or_else(|| detail.asset_index.as_ref().map(|i| i.id.clone()))
            .unwrap_or_else(|| "legacy".to_string());
        vars.insert("assets_index_name", assets_index_str.as_str());
        vars.insert("auth_uuid", account.uuid.as_str());
        let token = account
            .tokens
            .as_ref()
            .map(|t| t.access_token.as_str())
            .unwrap_or("0");
        vars.insert("auth_access_token", token);
        let has_token =
            account.account_type == AccountType::Microsoft && !token.is_empty() && token != "0";
        let auth_session = format!("{}:{}", token, account.uuid);
        vars.insert("auth_session", auth_session.as_str());
        let is_legacy_args = detail.arguments.is_none();
        let user_type = if account.account_type == AccountType::Microsoft {
            if is_legacy_args {
                "mojang"
            } else {
                "msa"
            }
        } else {
            "legacy"
        };
        vars.insert("user_type", user_type);
        vars.insert("user_properties", "{}");
        vars.insert("launcher_name", "MCLauncher");
        vars.insert("launcher_version", "0.1.0");
        vars.insert("classpath", classpath_str.as_str());
        vars.insert("classpath_separator", CLASSPATH_SEPARATOR);
        let lib_dir_str = self.paths.libraries_dir.to_str().ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Path libraries mengandung karakter bukan UTF-8.")
        })?;
        vars.insert("library_directory", lib_dir_str);
        let natives_dir_str = natives_dir.to_str().ok_or_else(|| {
            LauncherErrorPayload::invalid_input("Path natives mengandung karakter bukan UTF-8.")
        })?;
        vars.insert("natives_directory", natives_dir_str);
        vars.insert("resolution_width", "854");
        vars.insert("resolution_height", "480");
        let client_id_str = if account.account_type == AccountType::Microsoft {
            "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb"
        } else {
            ""
        };
        vars.insert("clientid", client_id_str);
        let xuid_str = account.xuid.as_deref().unwrap_or("");
        vars.insert("auth_xuid", xuid_str);

        // 9. Bentuk argumen JVM
        let mut final_args = Vec::new();

        // Memory: Pastikan min_mb <= max_mb agar JVM tidak crash dengan "Incompatible minimum and maximum heap sizes"
        let (min_mb, max_mb) = compute_memory_bounds(&instance, &settings);
        final_args.push(format!("-Xms{}m", min_mb));
        final_args.push(format!("-Xmx{}m", max_mb));

        // Presets & argumen JVM per-instance
        final_args.extend(collect_jvm_args(&instance, &settings));

        // Native path
        final_args.push(format!("-Djava.library.path={}", natives_dir_str));

        let mut legacy_args_mode = false;
        let mut game_args = Vec::new();

        // Deteksi sistem operasi & arsitektur dinamis untuk evaluasi rules
        let current_os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "osx"
        } else {
            "linux"
        };

        let current_arch = if cfg!(target_arch = "x86_64") {
            "x64"
        } else if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86"
        };

        if let Some(ref args_obj) = detail.arguments {
            if let Some(jvm_val) = args_obj.get("jvm") {
                let expanded_jvm = expand_arguments_json(jvm_val, &vars, current_os, current_arch);
                final_args.extend(expanded_jvm);
            }
            if let Some(game_val) = args_obj.get("game") {
                game_args = expand_arguments_json(game_val, &vars, current_os, current_arch);
            }
        } else if let Some(ref legacy_args) = detail.minecraft_arguments {
            legacy_args_mode = true;
            let parts: Vec<&str> = legacy_args.split_whitespace().collect();
            for p in parts {
                game_args.push(crate::args::replace_variables(p, &vars));
            }
        }

        if legacy_args_mode {
            final_args.push("-cp".to_string());
            final_args.push(classpath_str.clone());
        }

        // Main class
        let main_class = detail
            .main_class
            .unwrap_or_else(|| "net.minecraft.client.main.Main".to_string());
        final_args.push(main_class);

        // Game args
        final_args.extend(game_args);

        // Quick connect server address jika ada
        if let Some(server) = server_address {
            let base_mc_version = detail.inherits_from.as_deref().unwrap_or(&detail.id);
            final_args.extend(build_quick_connect_args(base_mc_version, server));
        }

        // 10. SEC-02 & SC-04: Buat argfile aman jika has_token atau total panjang besar
        let validated_java = launcher_security::validate_java_executable_path(
            std::path::Path::new(&java_inst.path),
        )?;

        let total_chars: usize = final_args.iter().map(|a| a.len()).sum();
        let (exec_args, created_argfile) =
            if should_use_argfile(java_inst.major_version, total_chars, has_token) {
                let argfile_path = create_argfile(&final_args, &instance_dir)?;
                let argfile_str = argfile_path.to_str().ok_or_else(|| {
                    LauncherErrorPayload::invalid_input(
                        "Path argfile mengandung karakter bukan UTF-8.",
                    )
                })?;
                (vec![format!("@{}", argfile_str)], Some(argfile_path))
            } else {
                (final_args, None)
            };

        on_progress("Meluncurkan proses Minecraft...".to_string(), 5, 5);

        let running_map = Arc::clone(&self.running);
        let inst_id_clone = instance_id.to_string();
        let argfile_exit = created_argfile.clone();
        let inst_dir_exit = instance_dir.clone();
        let natives_dir_exit = natives_dir.clone();

        let wrapped_exit = move |code: Option<i32>| {
            if let Some(ref argfile) = argfile_exit {
                let _ = launcher_security::safe_remove_file_within(&inst_dir_exit, argfile);
            }
            let _ =
                launcher_security::safe_remove_dir_all_within(&inst_dir_exit, &natives_dir_exit);
            let map = Arc::clone(&running_map);
            let id = inst_id_clone.clone();
            tokio::spawn(async move {
                map.write().await.remove(&id);
            });
            on_exit(code);
        };

        let running_game =
            match spawn_game_process(&validated_java, &exec_args, &game_dir, on_log, wrapped_exit)
                .await
            {
                Ok(rg) => {
                    natives_guard.disarm();
                    rg
                }
                Err(e) => {
                    if let Some(ref argfile) = created_argfile {
                        let _ = launcher_security::safe_remove_file_within(&instance_dir, argfile);
                    }
                    return Err(e);
                }
            };

        // Bersihkan argfile setelah proses berhasil di-spawn agar tidak tertinggal di disk (SC-05)
        if let Some(argfile) = created_argfile {
            let inst_dir_bg = instance_dir.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let _ = launcher_security::safe_remove_file_within(&inst_dir_bg, &argfile);
            });
        }

        let pid = running_game.pid;
        slot_guard.disarm();
        self.running.write().await.insert(
            instance_id.to_string(),
            LaunchSlot::Running(Arc::new(running_game)),
        );

        if let Ok(mut updated_inst) = self.instances.get(instance_id) {
            updated_inst.last_played_at = Some(chrono::Utc::now().to_rfc3339());
            if let Err(e) = self.instances.update(updated_inst) {
                tracing::warn!(
                    "Gagal menyimpan last_played_at instance {}: {:?}",
                    instance_id,
                    e
                );
            }
        }

        // Pangkas memori kerja launcher saat game berjalan (SEC-04)
        launcher_platform::trim_working_set();

        Ok(pid)
    }

    fn prepare_skin_resourcepack(
        &self,
        account: &launcher_types::Account,
        game_dir: &std::path::Path,
    ) {
        let is_slim = account
            .skin_variant
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case("slim"))
            .unwrap_or(false);

        let skins_dir = self.paths.root.join("skins");
        let read_cached_skin = || -> Option<Vec<u8>> {
            launcher_security::validate_instance_id(&account.id).ok()?;
            let cached =
                launcher_security::safe_subpath(&skins_dir, &format!("{}.png", account.id)).ok()?;
            launcher_security::ensure_descendants_no_symlink(&skins_dir, &cached).ok()?;
            std::fs::read(cached).ok()
        };

        let skin_bytes = if let Some(ref skin_source) = account.skin_url {
            if skin_source.starts_with("data:image/") {
                skin_source
                    .split_once(',')
                    .and_then(|(_, b64)| decode_base64(b64))
            } else if skin_source.starts_with("http://") || skin_source.starts_with("https://") {
                read_cached_skin()
            } else {
                // Tolak pembacaan path file lokal arbitrer (PS-02)
                None
            }
        } else {
            read_cached_skin()
        };

        let final_skin_bytes =
            skin_bytes.unwrap_or_else(|| launcher_archive::get_default_skin_bytes(is_slim));

        if final_skin_bytes.is_empty() {
            return;
        }

        let rp_dir = game_dir.join("resourcepacks");
        let rp_zip = rp_dir.join("SkinLoader-ResourcePack.zip");
        let _ = launcher_archive::build_skin_resourcepack(&final_skin_bytes, &rp_zip);

        let options_file = game_dir.join("options.txt");
        let _ = launcher_archive::activate_resourcepack_in_options(
            &options_file,
            "SkinLoader-ResourcePack.zip",
        );
    }

    #[doc(hidden)]
    pub async fn prepare_skin_artifacts(
        &self,
        instance: &Instance,
        account: &launcher_types::Account,
        game_dir: &std::path::Path,
        cancelled: &Arc<AtomicBool>,
    ) -> Result<(), LauncherErrorPayload> {
        if account.account_type != launcher_types::AccountType::Offline {
            return Ok(());
        }

        let skins_dir = self.paths.root.join("skins");
        let cached_skin_exists = launcher_security::validate_instance_id(&account.id).is_ok()
            && launcher_security::safe_subpath(&skins_dir, &format!("{}.png", account.id))
                .map(|p| p.exists())
                .unwrap_or(false);
        let has_custom_skin = account.skin_url.is_some() || cached_skin_exists;

        if has_custom_skin {
            self.prepare_skin_resourcepack(account, game_dir);
        }

        let has_csl_mod = instance.loader.loader_type != launcher_types::LoaderType::Vanilla
            && std::fs::read_dir(game_dir.join("mods"))
                .map(|entries| {
                    entries.flatten().any(|e| {
                        let name = e.file_name().to_string_lossy().to_lowercase();
                        name.contains("customskinloader") && name.ends_with(".jar")
                    })
                })
                .unwrap_or(false);

        if has_custom_skin || has_csl_mod {
            self.prepare_custom_skin_loader(account, game_dir, cancelled)
                .await?;
        }

        Ok(())
    }

    async fn prepare_custom_skin_loader(
        &self,
        account: &launcher_types::Account,
        game_dir: &std::path::Path,
        cancelled: &Arc<AtomicBool>,
    ) -> Result<(), LauncherErrorPayload> {
        let is_slim = account
            .skin_variant
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case("slim"))
            .unwrap_or(false);
        let model_str = if is_slim { "slim" } else { "default" };

        launcher_security::validate_instance_id(&account.id)?;
        launcher_security::validate_single_filename(&account.username)?;

        let csl_dir = game_dir.join("CustomSkinLoader");
        let local_skins_dir = csl_dir.join("LocalSkin").join("skins");
        launcher_security::ensure_dir_within(game_dir, &local_skins_dir, false)?;

        let user_skin_path = launcher_security::safe_subpath(
            &local_skins_dir,
            &format!("{}.png", account.username),
        )?;
        let user_skin_lower_path = launcher_security::safe_subpath(
            &local_skins_dir,
            &format!("{}.png", account.username.to_lowercase()),
        )?;

        // 1. Sinkronisasi skin lokal (simpan nama asli, lowercase, serta fallback Steve/Alex/default)
        let copy_targets = |src: &std::path::Path| {
            let _ = std::fs::copy(src, &user_skin_path);
            let _ = std::fs::copy(src, &user_skin_lower_path);
            let _ = std::fs::copy(src, local_skins_dir.join("Steve.png"));
            let _ = std::fs::copy(src, local_skins_dir.join("Alex.png"));
            let _ = std::fs::copy(src, local_skins_dir.join("default.png"));
        };

        let write_targets = |bytes: &[u8]| {
            let _ = std::fs::write(&user_skin_path, bytes);
            let _ = std::fs::write(&user_skin_lower_path, bytes);
            let _ = std::fs::write(local_skins_dir.join("Steve.png"), bytes);
            let _ = std::fs::write(local_skins_dir.join("Alex.png"), bytes);
            let _ = std::fs::write(local_skins_dir.join("default.png"), bytes);
        };

        let is_valid_png_file = |path: &std::path::Path| -> bool {
            const MAX_TEXTURE_BYTES: u64 = 2 * 1024 * 1024;
            const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
            let Ok(meta) = std::fs::metadata(path) else {
                return false;
            };
            if meta.len() < PNG_MAGIC.len() as u64 || meta.len() > MAX_TEXTURE_BYTES {
                return false;
            }
            let Ok(mut f) = std::fs::File::open(path) else {
                return false;
            };
            let mut magic = [0u8; 8];
            std::io::Read::read_exact(&mut f, &mut magic).is_ok() && magic == PNG_MAGIC
        };
        let is_valid_png_bytes = |bytes: &[u8]| -> bool {
            const MAX_TEXTURE_BYTES: usize = 2 * 1024 * 1024;
            const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
            bytes.len() <= MAX_TEXTURE_BYTES && bytes.starts_with(&PNG_MAGIC)
        };

        let skins_dir = self.paths.root.join("skins");
        let cached_skin =
            launcher_security::safe_subpath(&skins_dir, &format!("{}.png", account.id))?;

        if let Some(ref skin_source) = account.skin_url {
            if skin_source.starts_with("data:image/") {
                if let Some((_, b64)) = skin_source.split_once(',') {
                    if let Some(bytes) = decode_base64(b64) {
                        if is_valid_png_bytes(&bytes) {
                            write_targets(&bytes);
                        }
                    }
                }
            } else if skin_source.starts_with("https://") || skin_source.starts_with("http://") {
                if cached_skin.exists()
                    && launcher_security::ensure_descendants_no_symlink(&skins_dir, &cached_skin)
                        .is_ok()
                {
                    copy_targets(&cached_skin);
                } else {
                    let https_url = if skin_source.starts_with("http://") {
                        skin_source.replacen("http://", "https://", 1)
                    } else {
                        skin_source.clone()
                    };
                    let task = launcher_download::DownloadTask {
                        url: https_url,
                        destination: user_skin_path.clone(),
                        expected_sha1: None,
                        expected_sha256: None,
                        expected_size: None,
                    };
                    let _ = self
                        .downloader
                        .download_all(vec![task], |_| {}, Arc::clone(cancelled))
                        .await;
                    if user_skin_path.exists() {
                        if is_valid_png_file(&user_skin_path) {
                            copy_targets(&user_skin_path);
                        } else {
                            let _ = std::fs::remove_file(&user_skin_path);
                        }
                    }
                }
            } else if cached_skin.exists()
                && launcher_security::ensure_descendants_no_symlink(&skins_dir, &cached_skin)
                    .is_ok()
            {
                copy_targets(&cached_skin);
            }
        } else {
            let default_bytes = launcher_archive::get_default_skin_bytes(is_slim);
            write_targets(&default_bytes);
        }

        // 2. Sinkronisasi cape lokal
        let local_capes_dir = csl_dir.join("LocalSkin").join("capes");
        launcher_security::ensure_dir_within(game_dir, &local_capes_dir, false)?;
        let target_cape = launcher_security::safe_subpath(
            &local_capes_dir,
            &format!("{}.png", account.username),
        )?;
        let target_cape_lower = launcher_security::safe_subpath(
            &local_capes_dir,
            &format!("{}.png", account.username.to_lowercase()),
        )?;
        let cached_cape =
            launcher_security::safe_subpath(&skins_dir, &format!("{}_cape.png", account.id))?;
        let cached_cape_ok = cached_cape.exists()
            && launcher_security::ensure_descendants_no_symlink(&skins_dir, &cached_cape).is_ok();

        if let Some(ref cape_source) = account.cape_url {
            if cape_source.starts_with("data:image/") {
                if let Some((_, b64)) = cape_source.split_once(',') {
                    if let Some(bytes) = decode_base64(b64) {
                        if is_valid_png_bytes(&bytes) {
                            let _ = std::fs::write(&target_cape, &bytes);
                            let _ = std::fs::write(&target_cape_lower, &bytes);
                            let _ = std::fs::write(local_capes_dir.join("default.png"), &bytes);
                        }
                    }
                }
            } else if cached_cape_ok {
                let _ = std::fs::copy(&cached_cape, &target_cape);
                let _ = std::fs::copy(&cached_cape, &target_cape_lower);
                let _ = std::fs::copy(&cached_cape, local_capes_dir.join("default.png"));
            } else if cape_source.starts_with("https://") || cape_source.starts_with("http://") {
                let https_url = if cape_source.starts_with("http://") {
                    cape_source.replacen("http://", "https://", 1)
                } else {
                    cape_source.clone()
                };
                let task = launcher_download::DownloadTask {
                    url: https_url,
                    destination: target_cape.clone(),
                    expected_sha1: None,
                    expected_sha256: None,
                    expected_size: None,
                };
                let _ = self
                    .downloader
                    .download_all(vec![task], |_| {}, Arc::clone(cancelled))
                    .await;
                if target_cape.exists() {
                    if is_valid_png_file(&target_cape) {
                        let _ = std::fs::copy(&target_cape, &target_cape_lower);
                        let _ = std::fs::copy(&target_cape, local_capes_dir.join("default.png"));
                    } else {
                        let _ = std::fs::remove_file(&target_cape);
                    }
                }
            }
        } else if cached_cape_ok {
            let _ = std::fs::copy(&cached_cape, &target_cape);
            let _ = std::fs::copy(&cached_cape, &target_cape_lower);
            let _ = std::fs::copy(&cached_cape, local_capes_dir.join("default.png"));
        }

        // 3. Konfigurasi bawaan CustomSkinLoader (jangan timpa konfigurasi milik pengguna)
        let config_file = csl_dir.join("CustomSkinLoader.json");
        if !config_file.exists() {
            let default_config = serde_json::json!({
                "version": "14.15",
                "loadlist": [
                    {
                        "name": "LocalSkin",
                        "type": "Legacy",
                        "checkPNG": false,
                        "skin": "LocalSkin/skins/{USERNAME}.png",
                        "model": model_str,
                        "cape": "LocalSkin/capes/{USERNAME}.png"
                    },
                    {
                        "name": "Mojang",
                        "type": "Mojang"
                    },
                    {
                        "name": "LittleSkin",
                        "type": "Custom",
                        "root": "https://littleskin.cn/csl/"
                    },
                    {
                        "name": "ElyBy",
                        "type": "Custom",
                        "root": "https://skin.ely.by/csl/"
                    }
                ]
            });
            let _ = std::fs::write(
                &config_file,
                serde_json::to_string_pretty(&default_config).unwrap_or_default(),
            );
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn resolve_java_for_launch<F>(
        &self,
        instance: &Instance,
        settings: &Settings,
        required_major: u32,
        java_component: Option<&str>,
        emit_progress: bool,
        on_progress: &F,
        cancelled: Arc<AtomicBool>,
    ) -> Result<JavaInstallation, LauncherErrorPayload>
    where
        F: Fn(String, u32, u32) + Send + Sync + 'static,
    {
        if instance.java.mode == InstanceJavaMode::Custom {
            let custom_path = instance
                .java
                .path
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    LauncherErrorPayload::invalid_input(
                        "Path Java kustom instance belum ditentukan.",
                    )
                })?;
            return self.java.validate(custom_path);
        }

        if let Some(global_custom) = settings
            .java
            .custom_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return self.java.validate(global_custom);
        }

        let java_installations = self.java.detect();
        if settings.java.prefer_managed {
            if let Some(managed) = java_installations
                .iter()
                .find(|j| j.managed && j.major_version == required_major)
                .cloned()
            {
                return Ok(managed);
            }

            if cancelled.load(Ordering::Relaxed) {
                return Err(LauncherErrorPayload::internal("Peluncuran dibatalkan."));
            }

            if emit_progress {
                on_progress(
                    format!("Mengunduh runtime Java {} resmi Mojang...", required_major),
                    4,
                    5,
                );
            }

            return self
                .java
                .ensure(
                    required_major,
                    java_component,
                    &self.downloader,
                    |_| {},
                    cancelled,
                )
                .await;
        }

        if let Some(matching) = java_installations
            .iter()
            .find(|j| !j.managed && j.major_version == required_major)
            .or_else(|| {
                java_installations
                    .iter()
                    .find(|j| j.major_version == required_major)
            })
            .cloned()
        {
            return Ok(matching);
        }

        Err(LauncherErrorPayload::not_found(format!(
            "Java {} tidak ditemukan dan pengunduhan Java terkelola dinonaktifkan.",
            required_major
        )))
    }
}

pub(crate) fn compute_memory_bounds(instance: &Instance, settings: &Settings) -> (u32, u32) {
    let (raw_min, raw_max) = if instance.memory.override_global {
        (
            instance
                .memory
                .min_mb
                .unwrap_or(settings.memory.default_min_mb),
            instance
                .memory
                .max_mb
                .unwrap_or(settings.memory.default_max_mb),
        )
    } else {
        (
            settings.memory.default_min_mb,
            settings.memory.default_max_mb,
        )
    };

    crate::args::clamp_heap_memory(raw_min, raw_max)
}

pub(crate) fn collect_jvm_args(instance: &Instance, settings: &Settings) -> Vec<String> {
    let mut args = match instance.jvm_args.preset {
        JvmPreset::Default => {
            get_preset_jvm_args(settings.java.jvm_preset, &settings.java.custom_jvm_args)
        }
        JvmPreset::Custom => Vec::new(),
        override_preset => get_preset_jvm_args(override_preset, ""),
    };

    for item in &instance.jvm_args.extra {
        args.extend(get_preset_jvm_args(JvmPreset::Custom, item));
    }

    args
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let clean = input.trim_matches(|c: char| c.is_whitespace() || c == '=');
    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in clean.as_bytes() {
        let val = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

fn supports_quick_play(mc_version: &str) -> bool {
    let trimmed = mc_version.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Minecraft snapshot format: YYwWWx (quickPlayMultiplayer added in 23w14a for 1.20)
    if let Some((year_str, rest)) = trimmed.split_once('w') {
        if year_str.len() == 2 && year_str.chars().all(|c| c.is_ascii_digit()) {
            let week_digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let (Ok(year), Ok(week)) = (year_str.parse::<u32>(), week_digits.parse::<u32>()) {
                return year > 23 || (year == 23 && week >= 14);
            }
        }
    }

    // Release / pre-release / modded prefixes such as "1.20.4", "1.20-pre1", "1.20.4-forge-..."
    let core = trimmed.split(['-', '_', '+']).next().unwrap_or(trimmed);
    let mut parts = core.split('.');
    let major = parts.next().and_then(|p| p.parse::<u32>().ok());
    let minor = parts.next().and_then(|p| p.parse::<u32>().ok());
    match (major, minor) {
        (Some(maj), _) if maj > 1 => true,
        (Some(1), Some(min)) => min >= 20,
        _ => false,
    }
}

fn split_legacy_server_address(server: &str) -> (&str, Option<&str>) {
    if let Some(after_open) = server.strip_prefix('[') {
        if let Some((host, rest)) = after_open.split_once(']') {
            if let Some(port) = rest.strip_prefix(':') {
                if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
                    return (host, Some(port));
                }
            }
            return (host, None);
        }
    }

    if let Some((host, port)) = server.rsplit_once(':') {
        // Bare unbracketed IPv6 address (contains additional ':') must not be split as host:port
        if !host.contains(':') && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return (host, Some(port));
        }
    }

    (server, None)
}

pub fn build_quick_connect_args(mc_version: &str, server_address: &str) -> Vec<String> {
    let trimmed = server_address.trim();
    if trimmed.is_empty()
        || trimmed.len() > 255
        || trimmed.starts_with('-')
        || trimmed.starts_with('@')
        || trimmed
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\0')
    {
        return Vec::new();
    }

    if supports_quick_play(mc_version) {
        return vec!["--quickPlayMultiplayer".to_string(), trimmed.to_string()];
    }

    match split_legacy_server_address(trimmed) {
        (host, Some(port)) => vec![
            "--server".to_string(),
            host.to_string(),
            "--port".to_string(),
            port.to_string(),
        ],
        (host, None) => vec!["--server".to_string(), host.to_string()],
    }
}

struct NativesDirGuard {
    target: Option<(std::path::PathBuf, std::path::PathBuf)>,
}

impl NativesDirGuard {
    fn new(instance_dir: std::path::PathBuf, natives_dir: std::path::PathBuf) -> Self {
        Self {
            target: Some((instance_dir, natives_dir)),
        }
    }

    fn disarm(&mut self) {
        self.target = None;
    }
}

impl Drop for NativesDirGuard {
    fn drop(&mut self) {
        if let Some((instance_dir, natives_dir)) = self.target.take() {
            let _ = launcher_security::safe_remove_dir_all_within(&instance_dir, &natives_dir);
        }
    }
}
