use crate::logs::LauncherLogService;
use launcher_auth::MicrosoftAuthService;
use launcher_content::ContentService;
use launcher_discord::DiscordRpcService;
use launcher_download::DownloadManager;
use launcher_java::JavaService;
use launcher_launch::LaunchService;
use launcher_loader::LoaderService;
use launcher_platform::{DataPaths, SecretStore};
use launcher_servers::ServerService;
use launcher_storage::{AccountsStore, InstancesStore, SettingsStore};
use launcher_types::UpdateCheckResult;
use launcher_version::VersionService;
use tokio::sync::RwLock;

pub struct AppState {
    pub paths: DataPaths,
    pub settings: SettingsStore,
    pub instances: InstancesStore,
    pub accounts: AccountsStore,
    pub versions: VersionService,
    pub loaders: LoaderService,
    pub downloader: DownloadManager,
    pub java: JavaService,
    pub launch: LaunchService,
    pub servers: ServerService,
    pub content: ContentService,
    pub secrets: SecretStore,
    pub auth: MicrosoftAuthService,
    pub discord: DiscordRpcService,
    pub logs: LauncherLogService,
    pub verified_update: RwLock<Option<UpdateCheckResult>>,
}

impl AppState {
    pub fn new() -> Self {
        let paths = DataPaths::default_from_env();
        let logs = LauncherLogService::new(&paths);
        let settings = SettingsStore::new(paths.clone());
        let instances = InstancesStore::new(paths.clone());
        let accounts = AccountsStore::new(paths.clone());
        let versions = VersionService::new(paths.clone());
        let loaders = LoaderService::new(paths.clone(), versions.clone());
        let java = JavaService::new(paths.clone());
        let parallelism = settings.get().downloads.parallelism as usize;
        let downloader = DownloadManager::new(parallelism);

        let secrets = SecretStore::new(paths.secrets_file.clone());

        let launch = LaunchService::new(
            paths.clone(),
            versions.clone(),
            loaders.clone(),
            downloader.clone(),
            java.clone(),
            settings.clone(),
            instances.clone(),
            accounts.clone(),
            secrets.clone(),
        );

        let servers = ServerService::new(paths.clone());
        let content = ContentService::with_secrets(
            paths.clone(),
            downloader.clone(),
            instances.clone(),
            secrets.clone(),
        );

        let auth = MicrosoftAuthService::new(paths.clone(), accounts.clone(), secrets.clone());
        let discord = DiscordRpcService::new(settings.get().discord);

        Self {
            paths,
            settings,
            instances,
            accounts,
            versions,
            loaders,
            downloader,
            java,
            launch,
            servers,
            content,
            secrets,
            auth,
            discord,
            logs,
            verified_update: RwLock::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
