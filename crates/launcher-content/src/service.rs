use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use launcher_download::{DownloadManager, DownloadTask};
use launcher_net::HttpClient;
use launcher_platform::{DataPaths, SecretStore};
use launcher_security::validate_instance_id;
use launcher_storage::InstancesStore;
use launcher_types::{
    ContentFilter, ContentInstallPlan, ContentProject, ContentProviderId, ContentType,
    ContentUpdateInfo, ContentVersion, ErrorCode, Instance, InstanceContentEntry,
    IntegrationStatus, IntegrationsStatus, LauncherErrorPayload, SavedWorld, SearchQuery,
    SearchResult,
};

use crate::curseforge::{
    get_curseforge_project_with_base, list_curseforge_versions_with_base,
    search_curseforge_with_base, to_content_version as cf_to_version, API_BASE as CF_API_BASE,
};
use crate::modrinth::{
    get_modrinth_project, list_modrinth_versions, search_modrinth,
    to_content_version as mr_to_version, API_BASE as MR_API_BASE,
};
use crate::plan::build_install_plan_with_bases;

pub fn builtin_curseforge_proxy_url() -> Option<String> {
    option_env!("MCLAUNCHER_DEFAULT_CF_PROXY")
        .or(option_env!("CURSEFORGE_PROXY_URL"))
        .and_then(normalize_curseforge_proxy_url)
}

fn normalize_curseforge_proxy_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(trimmed).ok()?;
    let allowed = match parsed.scheme() {
        "https" => true,
        #[cfg(any(test, debug_assertions))]
        "http" => matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        _ => false,
    };
    if !allowed {
        return None;
    }
    let without_trailing_slash = parsed.as_str().trim_end_matches('/');
    if without_trailing_slash.ends_with("/v1") {
        Some(without_trailing_slash.to_string())
    } else {
        Some(format!("{}/v1", without_trailing_slash))
    }
}

#[derive(Clone)]
pub struct ContentService {
    paths: DataPaths,
    http: HttpClient,
    downloader: DownloadManager,
    instances: InstancesStore,
    secrets: SecretStore,
}

impl ContentService {
    pub fn new(paths: DataPaths, downloader: DownloadManager, instances: InstancesStore) -> Self {
        let secrets = SecretStore::new(paths.secrets_file.clone());
        Self::with_secrets(paths, downloader, instances, secrets)
    }

    pub fn with_secrets(
        paths: DataPaths,
        downloader: DownloadManager,
        instances: InstancesStore,
        secrets: SecretStore,
    ) -> Self {
        Self::with_http_client(paths, HttpClient::new(), downloader, instances, secrets)
    }

    pub fn with_http_client(
        paths: DataPaths,
        http: HttpClient,
        downloader: DownloadManager,
        instances: InstancesStore,
        secrets: SecretStore,
    ) -> Self {
        Self {
            paths,
            http,
            downloader,
            instances,
            secrets,
        }
    }

    fn integrations_file(&self) -> PathBuf {
        self.paths.root.join("integrations.json")
    }

    fn read_integration_string(&self, key: &str) -> Option<String> {
        let file = self.integrations_file();
        if !file.exists() {
            return None;
        }
        let contents = fs::read_to_string(&file).ok()?;
        let cfg = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
        cfg.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn stored_curseforge_key(&self) -> Option<String> {
        if let Some(secret) = self
            .secrets
            .get_sync("curseforgeApiKey")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            return Some(secret);
        }

        if let Some(legacy) = self.read_integration_string("curseforgeApiKey") {
            let _ = self.secrets.set_sync("curseforgeApiKey", &legacy);
            let _ = self.remove_legacy_curseforge_key_from_integrations();
            return Some(legacy);
        }

        None
    }

    pub fn get_curseforge_key(&self) -> Option<String> {
        if let Ok(env_key) = std::env::var("CURSEFORGE_API_KEY") {
            let trimmed = env_key.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        if let Some(stored) = self.stored_curseforge_key() {
            return Some(stored);
        }

        option_env!("CURSEFORGE_API_KEY")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
    }

    fn remove_legacy_curseforge_key_from_integrations(&self) -> Result<(), LauncherErrorPayload> {
        let file = self.integrations_file();
        if !file.exists() {
            return Ok(());
        }
        let Ok(contents) = fs::read_to_string(&file) else {
            return Ok(());
        };
        let Ok(mut current) = serde_json::from_str::<serde_json::Value>(&contents) else {
            return Ok(());
        };
        if let Some(obj) = current.as_object_mut() {
            if obj.remove("curseforgeApiKey").is_some() {
                launcher_storage::write_json_atomic(&file, &current)?;
            }
        }
        Ok(())
    }

    fn explicit_curseforge_proxy_url(&self) -> Option<String> {
        if let Ok(env_proxy) = std::env::var("CURSEFORGE_PROXY_URL") {
            if let Some(url) = normalize_curseforge_proxy_url(&env_proxy) {
                return Some(url);
            }
        }

        if let Some(stored_proxy) = self.read_integration_string("curseforgeProxyUrl") {
            if let Some(url) = normalize_curseforge_proxy_url(&stored_proxy) {
                return Some(url);
            }
        }

        None
    }

    pub fn get_curseforge_proxy_url(&self) -> Option<String> {
        self.explicit_curseforge_proxy_url()
            .or_else(builtin_curseforge_proxy_url)
    }

    fn curseforge_api_base(&self) -> String {
        if let Some(proxy) = self.explicit_curseforge_proxy_url() {
            proxy
        } else if self.get_curseforge_key().is_some() {
            CF_API_BASE.to_string()
        } else if let Some(builtin_proxy) = builtin_curseforge_proxy_url() {
            builtin_proxy
        } else {
            CF_API_BASE.to_string()
        }
    }

    pub fn set_curseforge_key(
        &self,
        key: Option<&str>,
    ) -> Result<IntegrationsStatus, LauncherErrorPayload> {
        let trimmed = key.map(|s| s.trim()).filter(|s| !s.is_empty());
        if let Some(val) = trimmed {
            self.secrets.set_sync("curseforgeApiKey", val)?;
        } else {
            self.secrets.delete_sync("curseforgeApiKey")?;
        }
        self.remove_legacy_curseforge_key_from_integrations()?;

        Ok(self.integrations_status())
    }

    pub fn set_azure_client_id(
        &self,
        id: Option<&str>,
    ) -> Result<IntegrationsStatus, LauncherErrorPayload> {
        let file = self.integrations_file();
        let trimmed = id.map(|s| s.trim()).filter(|s| !s.is_empty());

        let mut current: serde_json::Value = if file.exists() {
            fs::read_to_string(&file)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        if let Some(obj) = current.as_object_mut() {
            if let Some(val) = trimmed {
                obj.insert(
                    "msClientId".to_string(),
                    serde_json::Value::String(val.to_string()),
                );
            } else {
                obj.remove("msClientId");
            }
        }

        launcher_storage::write_json_atomic(&file, &current)?;

        Ok(self.integrations_status())
    }

    pub fn integrations_status(&self) -> IntegrationsStatus {
        let env_key = std::env::var("CURSEFORGE_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let stored_key = self.stored_curseforge_key();
        let explicit_proxy = self.explicit_curseforge_proxy_url();
        let builtin_proxy = builtin_curseforge_proxy_url();
        let builtin_key = option_env!("CURSEFORGE_API_KEY")
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let (configured, source) = if env_key.is_some() {
            (true, Some("env".to_string()))
        } else if stored_key.is_some() {
            (true, Some("settings".to_string()))
        } else if explicit_proxy.is_some() || builtin_proxy.is_some() {
            (true, Some("proxy".to_string()))
        } else if builtin_key.is_some() {
            (true, Some("builtin".to_string()))
        } else {
            (false, None)
        };

        let ms_env = std::env::var("MS_CLIENT_ID")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let ms_stored = self.read_integration_string("msClientId");

        let (ms_configured, ms_source) = if ms_env.is_some() {
            (true, Some("env".to_string()))
        } else if ms_stored.is_some() {
            (true, Some("settings".to_string()))
        } else {
            (true, Some("builtin".to_string()))
        };

        IntegrationsStatus {
            curseforge: IntegrationStatus {
                configured,
                source,
                stored: stored_key.is_some(),
            },
            azure: Some(IntegrationStatus {
                configured: ms_configured,
                source: ms_source,
                stored: ms_stored.is_some(),
            }),
        }
    }

    fn curseforge_api_key(&self) -> String {
        self.get_curseforge_key().unwrap_or_default()
    }

    fn safe_filename(name: &str) -> Result<String, LauncherErrorPayload> {
        launcher_security::validate_single_filename(name.trim()).map(|s| s.to_string())
    }

    pub(crate) fn content_folder(
        mc_dir: &Path,
        content_type: ContentType,
        world_id: Option<&str>,
    ) -> Result<PathBuf, LauncherErrorPayload> {
        match content_type {
            ContentType::Mod => Ok(mc_dir.join("mods")),
            ContentType::Resourcepack => Ok(mc_dir.join("resourcepacks")),
            ContentType::Shader => Ok(mc_dir.join("shaderpacks")),
            ContentType::Datapack => {
                let wid = world_id.ok_or_else(|| {
                    LauncherErrorPayload::invalid_input(
                        "Pilih world tujuan sebelum memasang datapack.",
                    )
                })?;
                let clean_wid = launcher_security::validate_single_filename(wid.trim())?;
                let saves_dir = mc_dir.join("saves");
                let world_dir = launcher_security::safe_subpath(&saves_dir, clean_wid)?;
                if world_dir.parent() != Some(saves_dir.as_path()) {
                    return Err(LauncherErrorPayload::invalid_input(
                        "Nama folder world tidak valid.",
                    ));
                }
                Ok(world_dir.join("datapacks"))
            }
            ContentType::World => Ok(mc_dir.join("saves")),
            ContentType::Modpack => Err(LauncherErrorPayload::invalid_input(
                "Modpack dipasang lewat impor modpack, bukan pemasangan berkas tunggal.",
            )),
        }
    }

    pub async fn search(&self, query: SearchQuery) -> Result<SearchResult, LauncherErrorPayload> {
        match query.provider {
            ContentProviderId::Modrinth => search_modrinth(&self.http, &query).await,
            ContentProviderId::Curseforge => {
                let key = self.curseforge_api_key();
                let api_base = self.curseforge_api_base();
                search_curseforge_with_base(&self.http, &api_base, &key, &query).await
            }
        }
    }

    pub async fn project(
        &self,
        provider: ContentProviderId,
        project_id: &str,
    ) -> Result<ContentProject, LauncherErrorPayload> {
        match provider {
            ContentProviderId::Modrinth => get_modrinth_project(&self.http, project_id).await,
            ContentProviderId::Curseforge => {
                let key = self.curseforge_api_key();
                let api_base = self.curseforge_api_base();
                get_curseforge_project_with_base(&self.http, &api_base, &key, project_id).await
            }
        }
    }

    pub async fn versions(
        &self,
        provider: ContentProviderId,
        project_id: &str,
        filter: ContentFilter,
    ) -> Result<Vec<ContentVersion>, LauncherErrorPayload> {
        match provider {
            ContentProviderId::Modrinth => {
                let list = list_modrinth_versions(&self.http, project_id, &filter).await?;
                let mut out = Vec::new();
                for d in list {
                    if let Ok(cv) = mr_to_version(&d) {
                        out.push(cv);
                    }
                }
                Ok(out)
            }
            ContentProviderId::Curseforge => {
                let key = self.curseforge_api_key();
                let api_base = self.curseforge_api_base();
                let list = list_curseforge_versions_with_base(
                    &self.http, &api_base, &key, project_id, &filter,
                )
                .await?;
                Ok(list.iter().map(cf_to_version).collect())
            }
        }
    }

    pub async fn plan(
        &self,
        instance_id: &str,
        provider: ContentProviderId,
        version_id: &str,
    ) -> Result<ContentInstallPlan, LauncherErrorPayload> {
        self.plan_with_world(instance_id, provider, version_id, None)
            .await
    }

    pub async fn plan_with_world(
        &self,
        instance_id: &str,
        provider: ContentProviderId,
        version_id: &str,
        world_id: Option<&str>,
    ) -> Result<ContentInstallPlan, LauncherErrorPayload> {
        let instance = self.instances.get(instance_id)?;
        let cf_key = self.curseforge_api_key();
        let api_base = self.curseforge_api_base();
        let mut plan = build_install_plan_with_bases(
            &self.http,
            &instance,
            provider,
            version_id,
            &cf_key,
            MR_API_BASE,
            &api_base,
        )
        .await?;

        let needs_world = plan.target.content_type == ContentType::Datapack
            || plan
                .dependencies
                .iter()
                .any(|d| d.content_type == ContentType::Datapack);
        if needs_world {
            if let Some(wid) = world_id.map(str::trim).filter(|s| !s.is_empty()) {
                let clean_wid = launcher_security::validate_single_filename(wid)?;
                let mc_dir =
                    launcher_security::safe_subpath(&self.paths.instances_dir, &instance.id)?
                        .join("minecraft");
                let saves_dir = mc_dir.join("saves");
                let world_dir = launcher_security::safe_subpath(&saves_dir, clean_wid)?;
                launcher_security::ensure_descendants_no_symlink(
                    &self.paths.instances_dir,
                    &world_dir,
                )?;
                if !world_dir.is_dir() || !world_dir.join("level.dat").is_file() {
                    return Err(LauncherErrorPayload::not_found(format!(
                        "World tujuan '{}' tidak ditemukan pada instance ini.",
                        clean_wid
                    )));
                }
            } else {
                plan.warnings.push(
                    "Datapack memerlukan pemilihan world tujuan sebelum pemasangan.".to_string(),
                );
            }
        }

        Ok(plan)
    }

    pub async fn install(
        &self,
        instance_id: &str,
        provider: ContentProviderId,
        version_id: &str,
        world_id: Option<&str>,
    ) -> Result<Instance, LauncherErrorPayload> {
        self.install_with_cancel(
            instance_id,
            provider,
            version_id,
            world_id,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    }

    pub async fn install_with_cancel(
        &self,
        instance_id: &str,
        provider: ContentProviderId,
        version_id: &str,
        world_id: Option<&str>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<Instance, LauncherErrorPayload> {
        check_install_cancelled(&cancel_flag)?;
        let plan = self
            .plan_with_world(instance_id, provider, version_id, world_id)
            .await?;
        self.install_plan_with_cancel(instance_id, plan, world_id, cancel_flag)
            .await
    }

    pub async fn install_plan(
        &self,
        instance_id: &str,
        plan: ContentInstallPlan,
        world_id: Option<&str>,
    ) -> Result<Instance, LauncherErrorPayload> {
        self.install_plan_with_cancel(
            instance_id,
            plan,
            world_id,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    }

    pub async fn install_plan_with_cancel(
        &self,
        instance_id: &str,
        plan: ContentInstallPlan,
        world_id: Option<&str>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<Instance, LauncherErrorPayload> {
        let mut instance = self.instances.get(instance_id)?;
        let mc_dir = self.paths.instances_dir.join(instance_id).join("minecraft");

        let mut to_install = vec![plan.target];
        to_install.extend(plan.dependencies);

        if let Some(blocked) = to_install
            .iter()
            .find(|entry| entry.version.download_url.is_none())
        {
            return Err(LauncherErrorPayload::distribution_blocked(format!(
                "Berkas '{}' ({}) tidak menyediakan tautan unduhan langsung dari penyedia.",
                blocked.version.file_name, blocked.version.project_id
            )));
        }

        enum RollbackArtifact {
            File { parent: PathBuf, path: PathBuf },
            Dir { parent: PathBuf, path: PathBuf },
        }
        enum DeferredRemoval {
            File { parent: PathBuf, path: PathBuf },
            Dir { parent: PathBuf, path: PathBuf },
        }

        let mut created_artifacts: Vec<RollbackArtifact> = Vec::new();
        let mut deferred_removals: Vec<DeferredRemoval> = Vec::new();

        let install_res: Result<(), LauncherErrorPayload> = async {
            for entry in to_install {
                check_install_cancelled(&cancel_flag)?;
                let v = entry.version;
                let url = v.download_url.ok_or_else(|| {
                    LauncherErrorPayload::distribution_blocked(format!(
                        "Berkas '{}' ({}) tidak menyediakan tautan unduhan langsung dari penyedia.",
                        v.file_name, v.project_id
                    ))
                })?;

                let clean_name = Self::safe_filename(&v.file_name)?;
                let target_dir = Self::content_folder(&mc_dir, entry.content_type, world_id)?;
                launcher_security::ensure_dir_within(&mc_dir, &target_dir, false)?;

                let mut recorded_world_folder = world_id.map(|s| s.to_string());

                if entry.content_type == ContentType::World {
                    let staging_dir = self
                        .paths
                        .root
                        .join("temp")
                        .join(format!("world-extract-{}", uuid::Uuid::new_v4()));
                    let archive_path = staging_dir.join(&clean_name);
                    let extract_dir = staging_dir.join("extracted");

                    let extract_res: Result<(String, PathBuf, bool), LauncherErrorPayload> =
                        async {
                            fs::create_dir_all(&staging_dir).map_err(|e| {
                                LauncherErrorPayload::file_system(format!(
                                    "Gagal membuat folder sementara world: {}",
                                    e
                                ))
                            })?;

                            self.downloader
                                .download_all(
                                    vec![DownloadTask {
                                        url,
                                        destination: archive_path.clone(),
                                        expected_sha1: v.sha1.clone(),
                                        expected_sha256: None,
                                        expected_size: None,
                                    }],
                                    |_| {},
                                    Arc::clone(&cancel_flag),
                                )
                                .await
                                .map_err(|e| map_cancel_err(&cancel_flag, e))?;

                            check_install_cancelled(&cancel_flag)?;
                            launcher_archive::extract_zip(&archive_path, &extract_dir, &[])?;

                            let world_root = find_world_root(&extract_dir).ok_or_else(|| {
                                LauncherErrorPayload::invalid_input(
                                    "Arsip world tidak mengandung berkas level.dat yang valid.",
                                )
                            })?;

                            let raw_folder_name = if world_root == extract_dir {
                                Path::new(&clean_name)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .filter(|s| !s.trim().is_empty())
                                    .unwrap_or("world")
                                    .to_string()
                            } else {
                                world_root
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .ok_or_else(|| {
                                        LauncherErrorPayload::invalid_input(
                                            "Nama folder world dalam arsip tidak valid.",
                                        )
                                    })?
                                    .to_string()
                            };

                            let folder_name = Self::safe_filename(&raw_folder_name)?;
                            let dest_world =
                                launcher_security::safe_subpath(&target_dir, &folder_name)?;
                            if dest_world.parent() != Some(target_dir.as_path()) {
                                return Err(LauncherErrorPayload::invalid_input(
                                    "Folder world tidak valid.",
                                ));
                            }

                            let existed_before = fs::symlink_metadata(&dest_world).is_ok();
                            if existed_before {
                                launcher_security::safe_remove_dir_all_within(
                                    &target_dir,
                                    &dest_world,
                                )?;
                            }

                            move_dir_contents_or_rename(&world_root, &dest_world)?;
                            Ok((folder_name, dest_world, existed_before))
                        }
                        .await;

                    let _ = launcher_security::safe_remove_dir_all_within(
                        &self.paths.root,
                        &staging_dir,
                    );
                    let (folder_name, dest_world, existed_before) = extract_res?;
                    if !existed_before {
                        created_artifacts.push(RollbackArtifact::Dir {
                            parent: target_dir.clone(),
                            path: dest_world,
                        });
                    }
                    recorded_world_folder = Some(folder_name);
                } else {
                    let dest_file = launcher_security::safe_subpath(&target_dir, &clean_name)?;
                    launcher_security::ensure_descendants_no_symlink(&target_dir, &dest_file)?;
                    let existed_before = fs::symlink_metadata(&dest_file).is_ok();

                    self.downloader
                        .download_all(
                            vec![DownloadTask {
                                url,
                                destination: dest_file.clone(),
                                expected_sha1: v.sha1.clone(),
                                expected_sha256: None,
                                expected_size: None,
                            }],
                            |_| {},
                            Arc::clone(&cancel_flag),
                        )
                        .await
                        .map_err(|e| map_cancel_err(&cancel_flag, e))?;

                    if !existed_before {
                        created_artifacts.push(RollbackArtifact::File {
                            parent: target_dir.clone(),
                            path: dest_file,
                        });
                    }
                }

                if let Some(pos) = instance
                    .content
                    .iter()
                    .position(|c| c.provider == v.provider && c.project_id == v.project_id)
                {
                    let old_entry = instance.content.remove(pos);
                    if entry.content_type == ContentType::World {
                        if old_entry.world_folder != recorded_world_folder {
                            if let Some(ref old_wf) = old_entry.world_folder {
                                if let Ok(clean_wf) =
                                    launcher_security::validate_single_filename(old_wf.trim())
                                {
                                    if let Ok(old_world_dir) =
                                        launcher_security::safe_subpath(&target_dir, clean_wf)
                                    {
                                        if old_world_dir.parent() == Some(target_dir.as_path()) {
                                            deferred_removals.push(DeferredRemoval::Dir {
                                                parent: target_dir.clone(),
                                                path: old_world_dir,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    } else if old_entry.file_name != clean_name {
                        // Queue stale JAR removal until metadata persistence succeeds.
                        if let Ok(clean_old) =
                            launcher_security::validate_single_filename(old_entry.file_name.trim())
                        {
                            if let Ok(old_file) =
                                launcher_security::safe_subpath(&target_dir, clean_old)
                            {
                                deferred_removals.push(DeferredRemoval::File {
                                    parent: target_dir.clone(),
                                    path: old_file,
                                });
                            }
                            if let Ok(old_disabled) = launcher_security::safe_subpath(
                                &target_dir,
                                &format!("{clean_old}.disabled"),
                            ) {
                                deferred_removals.push(DeferredRemoval::File {
                                    parent: target_dir.clone(),
                                    path: old_disabled,
                                });
                            }
                        }
                    }
                }

                instance.content.push(InstanceContentEntry {
                    provider: v.provider,
                    project_id: v.project_id,
                    version_id: v.id,
                    content_type: entry.content_type,
                    file_name: clean_name,
                    sha1: v.sha1.unwrap_or_default(),
                    world_folder: recorded_world_folder,
                    disabled: Some(false),
                });
            }

            self.instances.update(instance.clone())?;
            Ok(())
        }
        .await;

        if let Err(err) = install_res {
            for artifact in created_artifacts.into_iter().rev() {
                match artifact {
                    RollbackArtifact::File { parent, path } => {
                        let _ = launcher_security::safe_remove_file_within(&parent, &path);
                    }
                    RollbackArtifact::Dir { parent, path } => {
                        let _ = launcher_security::safe_remove_dir_all_within(&parent, &path);
                    }
                }
            }
            return Err(err);
        }

        for removal in deferred_removals {
            match removal {
                DeferredRemoval::File { parent, path } => {
                    let _ = launcher_security::safe_remove_file_within(&parent, &path);
                }
                DeferredRemoval::Dir { parent, path } => {
                    let _ = launcher_security::safe_remove_dir_all_within(&parent, &path);
                }
            }
        }

        Ok(instance)
    }

    pub fn remove(
        &self,
        instance_id: &str,
        provider: ContentProviderId,
        project_id: &str,
    ) -> Result<Instance, LauncherErrorPayload> {
        validate_instance_id(instance_id)?;
        let mut instance = self.instances.get(instance_id)?;
        let inst_dir = launcher_security::safe_subpath(&self.paths.instances_dir, instance_id)?;
        let mc_dir = inst_dir.join("minecraft");

        if let Some(pos) = instance
            .content
            .iter()
            .position(|c| c.provider == provider && c.project_id == project_id)
        {
            let entry = instance.content.remove(pos);
            let folder = match Self::content_folder(
                &mc_dir,
                entry.content_type,
                entry.world_folder.as_deref(),
            ) {
                Ok(f) => f,
                Err(_) => mc_dir.join("mods"),
            };

            if entry.content_type == ContentType::World {
                if let Some(ref wf) = entry.world_folder {
                    let clean_wf = launcher_security::validate_single_filename(wf.trim())?;
                    let world_dir = launcher_security::safe_subpath(&folder, clean_wf)?;
                    if world_dir.parent() == Some(folder.as_path())
                        && fs::symlink_metadata(&world_dir).is_ok()
                    {
                        launcher_security::safe_remove_dir_all_within(&folder, &world_dir)?;
                    }
                }
            }

            let clean_file = launcher_security::validate_single_filename(entry.file_name.trim())?;
            let file_normal = launcher_security::safe_subpath(&folder, clean_file)?;
            let file_disabled =
                launcher_security::safe_subpath(&folder, &format!("{clean_file}.disabled"))?;

            if fs::symlink_metadata(&file_normal).is_ok() {
                launcher_security::safe_remove_file_within(&folder, &file_normal)?;
            }
            if fs::symlink_metadata(&file_disabled).is_ok() {
                launcher_security::safe_remove_file_within(&folder, &file_disabled)?;
            }

            self.instances.update(instance.clone())?;
        }

        Ok(instance)
    }

    pub fn toggle(
        &self,
        instance_id: &str,
        provider: ContentProviderId,
        project_id: &str,
        enabled: bool,
    ) -> Result<Instance, LauncherErrorPayload> {
        validate_instance_id(instance_id)?;
        let mut instance = self.instances.get(instance_id)?;
        let inst_dir = launcher_security::safe_subpath(&self.paths.instances_dir, instance_id)?;
        let mc_dir = inst_dir.join("minecraft");

        if let Some(entry) = instance
            .content
            .iter_mut()
            .find(|c| c.provider == provider && c.project_id == project_id)
        {
            if entry.content_type != ContentType::Mod {
                return Err(LauncherErrorPayload::invalid_input(
                    "Hanya mod yang bisa dinonaktifkan.",
                ));
            }

            let clean_file = launcher_security::validate_single_filename(entry.file_name.trim())?;
            let mods_dir = mc_dir.join("mods");
            launcher_security::ensure_descendants_no_symlink(&self.paths.instances_dir, &mods_dir)?;
            let normal_path = launcher_security::safe_subpath(&mods_dir, clean_file)?;
            let disabled_path =
                launcher_security::safe_subpath(&mods_dir, &format!("{clean_file}.disabled"))?;

            let check_not_symlink = |p: &Path| -> Result<bool, LauncherErrorPayload> {
                match fs::symlink_metadata(p) {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() {
                            return Err(LauncherErrorPayload::invalid_input(
                                "Symlink tidak diizinkan pada berkas mod.",
                            ));
                        }
                        Ok(true)
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                    Err(e) => Err(LauncherErrorPayload::file_system(format!(
                        "Gagal memeriksa berkas mod: {}",
                        e
                    ))),
                }
            };

            if enabled {
                if check_not_symlink(&disabled_path)? {
                    let _ = check_not_symlink(&normal_path)?;
                    fs::rename(&disabled_path, &normal_path).map_err(|e| {
                        LauncherErrorPayload::file_system(format!("Gagal mengaktifkan mod: {}", e))
                    })?;
                }
                entry.disabled = Some(false);
            } else {
                if check_not_symlink(&normal_path)? {
                    let _ = check_not_symlink(&disabled_path)?;
                    fs::rename(&normal_path, &disabled_path).map_err(|e| {
                        LauncherErrorPayload::file_system(format!("Gagal menonaktifkan mod: {}", e))
                    })?;
                }
                entry.disabled = Some(true);
            }

            self.instances.update(instance.clone())?;
        }

        Ok(instance)
    }

    pub fn worlds(&self, instance_id: &str) -> Result<Vec<SavedWorld>, LauncherErrorPayload> {
        validate_instance_id(instance_id)?;
        let saves_dir = self
            .paths
            .instances_dir
            .join(instance_id)
            .join("minecraft")
            .join("saves");

        let mut worlds = Vec::new();
        if !saves_dir.exists() {
            return Ok(worlds);
        }

        let entries = fs::read_dir(&saves_dir).map_err(|e| {
            LauncherErrorPayload::file_system(format!("Gagal membaca saves: {}", e))
        })?;

        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let level_dat = entry.path().join("level.dat");
                if level_dat.exists() {
                    let folder_name = entry.file_name().to_string_lossy().to_string();
                    worlds.push(SavedWorld {
                        folder: folder_name.clone(),
                        name: folder_name,
                    });
                }
            }
        }

        Ok(worlds)
    }

    pub async fn check_updates(
        &self,
        instance_id: &str,
    ) -> Result<Vec<ContentUpdateInfo>, LauncherErrorPayload> {
        let instance = self.instances.get(instance_id)?;
        let instance_loader = match instance.loader.loader_type {
            launcher_types::LoaderType::Vanilla => None,
            l => Some(format!("{:?}", l).to_lowercase()),
        };

        use futures_util::stream::{self, StreamExt};

        let candidates: Vec<(ContentProviderId, String, String, ContentFilter)> = instance
            .content
            .iter()
            .filter(|entry| {
                let is_trackable = !matches!(
                    entry.content_type,
                    ContentType::Modpack | ContentType::World
                );
                is_trackable
                    && !entry.project_id.trim().is_empty()
                    && !entry.project_id.contains('/')
                    && !entry.project_id.contains('\\')
                    && entry.project_id != entry.version_id
            })
            .map(|entry| {
                let filter = ContentFilter {
                    minecraft_version: Some(instance.minecraft_version.clone()),
                    loader: match entry.content_type {
                        ContentType::Mod => instance_loader.clone(),
                        ContentType::Datapack => Some("datapack".to_string()),
                        ContentType::Resourcepack
                        | ContentType::Shader
                        | ContentType::Modpack
                        | ContentType::World => None,
                    },
                    featured: None,
                };
                (
                    entry.provider,
                    entry.project_id.clone(),
                    entry.version_id.clone(),
                    filter,
                )
            })
            .collect();

        let results: Vec<Option<ContentUpdateInfo>> = stream::iter(candidates)
            .map(|(provider, project_id, current_version_id, filter)| async move {
                let versions = self.versions(provider, &project_id, filter).await.ok()?;
                let latest = versions.first()?;
                if latest.id != current_version_id {
                    Some(ContentUpdateInfo {
                        provider,
                        project_id,
                        current_version_id,
                        latest_version_id: latest.id.clone(),
                        latest_version_number: latest.version_number.clone(),
                        file_name: latest.file_name.clone(),
                    })
                } else {
                    None
                }
            })
            .buffer_unordered(6)
            .collect()
            .await;

        let updates = results.into_iter().flatten().collect();

        Ok(updates)
    }

    pub async fn install_modpack<F>(
        &self,
        provider: ContentProviderId,
        version_id: &str,
        instance_name: Option<&str>,
        on_progress: F,
    ) -> Result<Instance, LauncherErrorPayload>
    where
        F: Fn(&str, u32, u32) + Send + Sync + Clone + 'static,
    {
        self.install_modpack_with_cancel(
            provider,
            version_id,
            instance_name,
            on_progress,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    }

    pub async fn install_modpack_with_cancel<F>(
        &self,
        provider: ContentProviderId,
        version_id: &str,
        instance_name: Option<&str>,
        on_progress: F,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<Instance, LauncherErrorPayload>
    where
        F: Fn(&str, u32, u32) + Send + Sync + Clone + 'static,
    {
        self.install_modpack_with_options(
            provider,
            version_id,
            instance_name,
            crate::modpack::ModpackInstallOptions::default(),
            on_progress,
            cancel_flag,
        )
        .await
    }

    pub async fn install_modpack_with_options<F>(
        &self,
        provider: ContentProviderId,
        version_id: &str,
        instance_name: Option<&str>,
        options: crate::modpack::ModpackInstallOptions,
        on_progress: F,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<Instance, LauncherErrorPayload>
    where
        F: Fn(&str, u32, u32) + Send + Sync + Clone + 'static,
    {
        let cf_key = self.curseforge_api_key();
        let api_base = self.curseforge_api_base();
        crate::modpack::install_modpack_with_cancel_and_options(
            &self.paths,
            &self.http,
            &self.downloader,
            &self.instances,
            provider,
            version_id,
            instance_name,
            &cf_key,
            &api_base,
            options,
            on_progress,
            cancel_flag,
        )
        .await
    }
}

fn check_install_cancelled(cancel_flag: &AtomicBool) -> Result<(), LauncherErrorPayload> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err(LauncherErrorPayload::new(
            ErrorCode::Cancelled,
            "Instalasi dibatalkan oleh pengguna.",
        ));
    }
    Ok(())
}

fn map_cancel_err(cancel_flag: &AtomicBool, err: LauncherErrorPayload) -> LauncherErrorPayload {
    if cancel_flag.load(Ordering::Relaxed) {
        LauncherErrorPayload::new(ErrorCode::Cancelled, "Instalasi dibatalkan oleh pengguna.")
    } else {
        err
    }
}

fn find_world_root(extract_dir: &Path) -> Option<PathBuf> {
    if extract_dir.join("level.dat").is_file() {
        return Some(extract_dir.to_path_buf());
    }

    let entries = fs::read_dir(extract_dir).ok()?;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if name.to_str() == Some("__MACOSX") {
                continue;
            }
            if path.join("level.dat").is_file() {
                return Some(path);
            }
            subdirs.push(path);
        }
    }

    if subdirs.len() == 1 {
        if let Ok(nested) = fs::read_dir(&subdirs[0]) {
            for entry in nested.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("level.dat").is_file() {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn move_dir_contents_or_rename(src: &Path, dst: &Path) -> Result<(), LauncherErrorPayload> {
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }

    copy_dir_recursive(src, dst).map_err(|e| {
        LauncherErrorPayload::file_system(format!("Gagal memindahkan folder world: {}", e))
    })
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(dst) {
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Symlink tidak diizinkan pada tujuan salin konten",
            ));
        }
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Symlink tidak diizinkan dalam direktori konten",
            ));
        }
        let name = entry.file_name();
        let name_str = name.to_str().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Nama berkas bukan UTF-8 yang valid",
            )
        })?;
        launcher_security::validate_single_filename(name_str)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.message))?;
        let src_path = entry.path();
        let dst_path = dst.join(name_str);
        if ft.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if ft.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
