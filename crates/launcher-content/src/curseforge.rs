use launcher_net::HttpClient;
use launcher_types::{
    ContentFilter, ContentProject, ContentProjectSummary, ContentProviderId, ContentSort,
    ContentType, ContentVersion, ErrorCode, LauncherErrorPayload, SearchQuery, SearchResult,
};
use serde::Deserialize;
use url::Url;

pub(crate) const API_BASE: &str = "https://api.curseforge.com/v1";
const GAME_ID: u32 = 432;

fn is_proxy_base(api_base: &str) -> bool {
    api_base.trim_end_matches('/') != API_BASE
}

async fn cf_get_json<T: serde::de::DeserializeOwned>(
    http: &HttpClient,
    url: &str,
    api_base: &str,
    api_key: &str,
    context: &str,
) -> Result<T, LauncherErrorPayload> {
    let using_proxy = is_proxy_base(api_base);
    if api_key.is_empty() && !using_proxy {
        return Err(LauncherErrorPayload::provider_not_configured(
            "API Key atau Proxy CurseForge belum dikonfigurasi.",
        ));
    }

    let mut req = http.request(reqwest::Method::GET, url)?;
    if !using_proxy && !api_key.is_empty() {
        req = req.header("x-api-key", api_key);
    }

    let res = req.send().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal menghubungi CurseForge saat {context}: {e}"))
    })?;
    let status = res.status();
    check_cf_status(status.is_success(), status.as_u16(), context)?;

    res.json::<T>().await.map_err(|e| {
        LauncherErrorPayload::network(format!(
            "Gagal membaca respon CurseForge saat {context}: {e}"
        ))
    })
}

fn check_cf_status(
    is_success: bool,
    status_code: u16,
    context: &str,
) -> Result<(), LauncherErrorPayload> {
    if is_success {
        return Ok(());
    }
    match status_code {
        429 => Err(LauncherErrorPayload::new(
            ErrorCode::RateLimited,
            format!("Batas permintaan CurseForge tercapai (HTTP 429) saat {context}."),
        )),
        401 | 403 => Err(LauncherErrorPayload::provider_not_configured(format!(
            "API Key CurseForge tidak valid atau ditolak (HTTP {status_code}) saat {context}."
        ))),
        404 => Err(LauncherErrorPayload::not_found(format!(
            "Resource CurseForge tidak ditemukan (HTTP 404) saat {context}."
        ))),
        _ => Err(LauncherErrorPayload::network(format!(
            "Permintaan CurseForge gagal (HTTP {status_code}) saat {context}."
        ))),
    }
}

pub fn class_id_for_type(t: ContentType) -> u32 {
    match t {
        ContentType::Mod => 6,
        ContentType::Modpack => 4471,
        ContentType::Resourcepack => 12,
        ContentType::Shader => 6552,
        ContentType::Datapack => 6945,
        ContentType::World => 17,
    }
}

pub fn type_from_class_id(class_id: u32) -> ContentType {
    match class_id {
        4471 => ContentType::Modpack,
        12 => ContentType::Resourcepack,
        6552 => ContentType::Shader,
        6945 => ContentType::Datapack,
        17 => ContentType::World,
        _ => ContentType::Mod,
    }
}

pub fn loader_type_id(loader: &str) -> Option<u32> {
    match loader.to_lowercase().as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct CfPagination {
    #[serde(rename = "totalCount")]
    total_count: u64,
}

#[derive(Debug, Deserialize)]
struct CfModLogo {
    url: Option<String>,
    #[serde(rename = "thumbnailUrl")]
    thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CfMod {
    id: u64,
    #[serde(rename = "classId")]
    class_id: Option<u32>,
    name: String,
    slug: String,
    summary: Option<String>,
    #[serde(rename = "downloadCount")]
    download_count: Option<u64>,
    #[serde(rename = "dateModified")]
    date_modified: Option<String>,
    logo: Option<CfModLogo>,
    categories: Option<Vec<CfCategory>>,
}

#[derive(Debug, Deserialize)]
struct CfCategory {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CfSearchResponse {
    data: Vec<CfMod>,
    pagination: CfPagination,
}

#[derive(Debug, Deserialize)]
struct CfModResponse {
    data: CfMod,
}

#[derive(Debug, Deserialize)]
struct CfDescriptionResponse {
    data: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfHash {
    pub value: String,
    pub algo: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfDependency {
    #[serde(rename = "modId")]
    pub mod_id: u64,
    #[serde(rename = "relationType")]
    pub relation_type: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfFile {
    pub id: u64,
    #[serde(rename = "modId")]
    pub mod_id: u64,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "releaseType")]
    pub release_type: u32,
    #[serde(rename = "fileDate")]
    pub file_date: Option<String>,
    #[serde(rename = "downloadCount")]
    pub download_count: Option<u64>,
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
    #[serde(default)]
    pub hashes: Vec<CfHash>,
    #[serde(rename = "gameVersions", default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<CfDependency>,
}

#[derive(Debug, Deserialize)]
struct CfFilesResponse {
    data: Vec<CfFile>,
}

#[derive(Debug, Deserialize)]
struct CfFileResponse {
    data: CfFile,
}

fn release_type_str(rt: u32) -> String {
    match rt {
        2 => "beta".to_string(),
        3 => "alpha".to_string(),
        _ => "release".to_string(),
    }
}

pub fn to_content_version(f: &CfFile) -> ContentVersion {
    let sha1 = f
        .hashes
        .iter()
        .find(|h| h.algo == 1)
        .map(|h| h.value.clone());
    let mut game_versions = Vec::new();
    let mut loaders = Vec::new();

    for gv in &f.game_versions {
        let is_version_num = gv.chars().all(|c| c.is_ascii_digit() || c == '.');
        if is_version_num {
            game_versions.push(gv.clone());
        } else {
            loaders.push(gv.to_lowercase());
        }
    }

    // Preserve null when CurseForge disallows third-party distribution; never synthesize CDN URLs.
    let download_url = f.download_url.clone();

    ContentVersion {
        provider: ContentProviderId::Curseforge,
        id: format!("{}:{}", f.mod_id, f.id),
        project_id: f.mod_id.to_string(),
        name: f.display_name.clone(),
        version_number: f.display_name.clone(),
        version_type: release_type_str(f.release_type),
        game_versions,
        loaders,
        downloads: f.download_count.unwrap_or(0),
        published_at: f.file_date.clone().unwrap_or_default(),
        file_name: f.file_name.clone(),
        download_url,
        sha1,
    }
}

pub async fn search_curseforge(
    http: &HttpClient,
    api_key: &str,
    query: &SearchQuery,
) -> Result<SearchResult, LauncherErrorPayload> {
    search_curseforge_with_base(http, API_BASE, api_key, query).await
}

pub(crate) async fn search_curseforge_with_base(
    http: &HttpClient,
    api_base: &str,
    api_key: &str,
    query: &SearchQuery,
) -> Result<SearchResult, LauncherErrorPayload> {
    let base = api_base.trim_end_matches('/');
    let mut url = Url::parse(&format!("{}/mods/search", base))
        .map_err(|e| LauncherErrorPayload::internal(format!("Invalid URL: {}", e)))?;

    let class_id = class_id_for_type(query.content_type).to_string();
    let index = ((query.page.saturating_sub(1)) * query.page_size).to_string();
    let page_size = query.page_size.to_string();

    url.query_pairs_mut()
        .append_pair("gameId", &GAME_ID.to_string())
        .append_pair("classId", &class_id)
        .append_pair("index", &index)
        .append_pair("pageSize", &page_size)
        .append_pair("searchFilter", &query.text);

    if let Some(ref gv) = query.minecraft_version {
        url.query_pairs_mut().append_pair("gameVersion", gv);
    }
    if let Some(ref l) = query.loader {
        if let Some(loader_id) = loader_type_id(l) {
            url.query_pairs_mut()
                .append_pair("modLoaderType", &loader_id.to_string());
        }
    }

    let sort_field = match query.sort {
        Some(ContentSort::Relevance) | None => "1",
        Some(ContentSort::Downloads) => "6",
        Some(ContentSort::Updated) => "3",
        Some(ContentSort::Newest) => "2",
        Some(ContentSort::Follows) => "1",
    };
    url.query_pairs_mut().append_pair("sortField", sort_field);
    url.query_pairs_mut().append_pair("sortOrder", "desc");

    let search_res: CfSearchResponse =
        cf_get_json(http, url.as_str(), base, api_key, "mencari konten").await?;

    let items = search_res
        .data
        .into_iter()
        .map(|m| ContentProjectSummary {
            provider: ContentProviderId::Curseforge,
            id: m.id.to_string(),
            slug: m.slug,
            title: m.name,
            description: m.summary.unwrap_or_default(),
            content_type: query.content_type,
            downloads: m.download_count.unwrap_or(0),
            icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)),
        })
        .collect();

    Ok(SearchResult {
        items,
        total: search_res.pagination.total_count,
        page: query.page,
        page_size: query.page_size,
    })
}

pub async fn get_curseforge_project(
    http: &HttpClient,
    api_key: &str,
    project_id: &str,
) -> Result<ContentProject, LauncherErrorPayload> {
    get_curseforge_project_with_base(http, API_BASE, api_key, project_id).await
}

pub(crate) async fn get_curseforge_project_with_base(
    http: &HttpClient,
    api_base: &str,
    api_key: &str,
    project_id: &str,
) -> Result<ContentProject, LauncherErrorPayload> {
    launcher_security::validate_safe_single_component(project_id)?;
    let base = api_base.trim_end_matches('/');
    let mod_url = format!("{}/mods/{}", base, project_id);
    let desc_url = format!("{}/mods/{}/description", base, project_id);

    let mod_req =
        cf_get_json::<CfModResponse>(http, &mod_url, base, api_key, "mengambil detail mod");
    let desc_req = cf_get_json::<CfDescriptionResponse>(
        http,
        &desc_url,
        base,
        api_key,
        "mengambil deskripsi mod",
    );

    let (mod_res, desc_res) = tokio::join!(mod_req, desc_req);

    let m = mod_res?;
    let body = desc_res.map(|d| d.data).unwrap_or_default();

    let categories = m
        .data
        .categories
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.name)
        .collect();

    Ok(ContentProject {
        summary: ContentProjectSummary {
            provider: ContentProviderId::Curseforge,
            id: m.data.id.to_string(),
            slug: m.data.slug,
            title: m.data.name,
            description: m.data.summary.unwrap_or_default(),
            content_type: m
                .data
                .class_id
                .map(type_from_class_id)
                .unwrap_or(ContentType::Mod),
            downloads: m.data.download_count.unwrap_or(0),
            icon_url: m.data.logo.and_then(|l| l.thumbnail_url.or(l.url)),
        },
        body,
        updated_at: m.data.date_modified.unwrap_or_default(),
        categories,
        loaders: Vec::new(),
        game_versions: Vec::new(),
    })
}

pub async fn list_curseforge_versions(
    http: &HttpClient,
    api_key: &str,
    project_id: &str,
    filter: &ContentFilter,
) -> Result<Vec<CfFile>, LauncherErrorPayload> {
    list_curseforge_versions_with_base(http, API_BASE, api_key, project_id, filter).await
}

pub(crate) async fn list_curseforge_versions_with_base(
    http: &HttpClient,
    api_base: &str,
    api_key: &str,
    project_id: &str,
    filter: &ContentFilter,
) -> Result<Vec<CfFile>, LauncherErrorPayload> {
    launcher_security::validate_safe_single_component(project_id)?;
    let base = api_base.trim_end_matches('/');
    let mut url = Url::parse(&format!("{}/mods/{}/files", base, project_id))
        .map_err(|e| LauncherErrorPayload::internal(format!("Invalid URL: {}", e)))?;

    if let Some(ref gv) = filter.minecraft_version {
        url.query_pairs_mut().append_pair("gameVersion", gv);
    }
    if let Some(ref l) = filter.loader {
        if let Some(loader_id) = loader_type_id(l) {
            url.query_pairs_mut()
                .append_pair("modLoaderType", &loader_id.to_string());
        }
    }
    url.query_pairs_mut().append_pair("pageSize", "50");

    let files_res: CfFilesResponse =
        cf_get_json(http, url.as_str(), base, api_key, "mengambil daftar versi").await?;

    Ok(files_res.data)
}

pub async fn get_curseforge_file(
    http: &HttpClient,
    api_key: &str,
    project_id: &str,
    file_id: u64,
) -> Result<CfFile, LauncherErrorPayload> {
    get_curseforge_file_with_base(http, API_BASE, api_key, project_id, file_id).await
}

pub(crate) async fn get_curseforge_file_with_base(
    http: &HttpClient,
    api_base: &str,
    api_key: &str,
    project_id: &str,
    file_id: u64,
) -> Result<CfFile, LauncherErrorPayload> {
    launcher_security::validate_safe_single_component(project_id)?;
    let base = api_base.trim_end_matches('/');
    let url = format!("{}/mods/{}/files/{}", base, project_id, file_id);
    let file_res: CfFileResponse =
        cf_get_json(http, &url, base, api_key, "mengambil detail file").await?;

    Ok(file_res.data)
}

pub(crate) async fn get_curseforge_content_type_with_base(
    http: &HttpClient,
    api_base: &str,
    api_key: &str,
    project_id: &str,
) -> Result<ContentType, LauncherErrorPayload> {
    launcher_security::validate_safe_single_component(project_id)?;
    let base = api_base.trim_end_matches('/');
    let url = format!("{}/mods/{}", base, project_id);
    let mod_res: CfModResponse =
        cf_get_json(http, &url, base, api_key, "mengambil tipe konten").await?;

    Ok(mod_res
        .data
        .class_id
        .map(type_from_class_id)
        .unwrap_or(ContentType::Mod))
}
