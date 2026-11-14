use launcher_net::HttpClient;
use launcher_types::{
    ContentFilter, ContentProject, ContentProjectSummary, ContentProviderId, ContentSort,
    ContentType, ContentVersion, LauncherErrorPayload, SearchQuery, SearchResult,
};
use serde::Deserialize;
use url::Url;

pub(crate) const API_BASE: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Deserialize)]
struct ModrinthSearchHit {
    project_id: String,
    project_type: String,
    slug: String,
    title: String,
    description: String,
    downloads: u64,
    icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthSearchHit>,
    total_hits: u64,
}

#[derive(Debug, Deserialize)]
struct ModrinthProjectResponse {
    id: String,
    slug: String,
    title: String,
    description: String,
    body: String,
    project_type: String,
    categories: Vec<String>,
    loaders: Vec<String>,
    game_versions: Vec<String>,
    downloads: u64,
    icon_url: Option<String>,
    updated: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthVersionFile {
    pub hashes: ModrinthHashes,
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthHashes {
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthDependencyEntry {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthVersionDetail {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: u64,
    pub date_published: String,
    pub files: Vec<ModrinthVersionFile>,
    pub dependencies: Vec<ModrinthDependencyEntry>,
}

fn type_from_str(s: &str, fallback: ContentType) -> ContentType {
    match s {
        "mod" => ContentType::Mod,
        "modpack" => ContentType::Modpack,
        "resourcepack" => ContentType::Resourcepack,
        "shader" => ContentType::Shader,
        "datapack" => ContentType::Datapack,
        "world" => ContentType::World,
        _ => fallback,
    }
}

fn primary_file(v: &ModrinthVersionDetail) -> Option<&ModrinthVersionFile> {
    v.files
        .iter()
        .find(|f| f.primary)
        .or_else(|| v.files.first())
}

pub fn to_content_version(
    v: &ModrinthVersionDetail,
) -> Result<ContentVersion, LauncherErrorPayload> {
    let file = primary_file(v).ok_or_else(|| {
        LauncherErrorPayload::not_found(format!(
            "Versi '{}' tidak memiliki berkas unduhan.",
            v.version_number
        ))
    })?;

    Ok(ContentVersion {
        provider: ContentProviderId::Modrinth,
        id: v.id.clone(),
        project_id: v.project_id.clone(),
        name: v.name.clone(),
        version_number: v.version_number.clone(),
        version_type: v.version_type.clone(),
        game_versions: v.game_versions.clone(),
        loaders: v.loaders.clone(),
        downloads: v.downloads,
        published_at: v.date_published.clone(),
        file_name: file.filename.clone(),
        download_url: Some(file.url.clone()),
        sha1: file.hashes.sha1.clone(),
    })
}

pub async fn search_modrinth(
    http: &HttpClient,
    query: &SearchQuery,
) -> Result<SearchResult, LauncherErrorPayload> {
    let mut url = Url::parse(&format!("{}/search", API_BASE))
        .map_err(|e| LauncherErrorPayload::internal(format!("Invalid URL: {}", e)))?;

    let type_str = match query.content_type {
        ContentType::Mod => "mod",
        ContentType::Modpack => "modpack",
        ContentType::Shader => "shader",
        ContentType::Resourcepack => "resourcepack",
        ContentType::Datapack => "datapack",
        ContentType::World => "world",
    };

    let mut facets = vec![vec![format!("project_type:{}", type_str)]];
    if let Some(ref mv) = query.minecraft_version {
        facets.push(vec![format!("versions:{}", mv)]);
    }
    if let Some(ref l) = query.loader {
        facets.push(vec![format!("categories:{}", l)]);
    }
    if let Some(ref cats) = query.categories {
        if !cats.is_empty() {
            facets.push(
                cats.iter()
                    .map(|c| format!("categories:{}", c.to_lowercase().replace(' ', "-")))
                    .collect(),
            );
        }
    }

    let facets_json = serde_json::to_string(&facets).unwrap_or_else(|_| "[]".into());
    let offset = (query.page.saturating_sub(1) * query.page_size).to_string();
    let limit = query.page_size.to_string();
    let index = match query.sort {
        Some(ContentSort::Relevance) => "relevance",
        Some(ContentSort::Downloads) => "downloads",
        Some(ContentSort::Follows) => "follows",
        Some(ContentSort::Updated) => "updated",
        Some(ContentSort::Newest) => "newest",
        None => {
            if query.text.is_empty() {
                "downloads"
            } else {
                "relevance"
            }
        }
    };

    url.query_pairs_mut()
        .append_pair("query", &query.text)
        .append_pair("limit", &limit)
        .append_pair("offset", &offset)
        .append_pair("index", index)
        .append_pair("facets", &facets_json);

    let res = http.get(url.as_str()).await?;
    let search_res: ModrinthSearchResponse = res.json().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal membaca hasil pencarian Modrinth: {}", e))
    })?;

    let items = search_res
        .hits
        .into_iter()
        .map(|hit| ContentProjectSummary {
            provider: ContentProviderId::Modrinth,
            id: hit.project_id,
            slug: hit.slug,
            title: hit.title,
            description: hit.description,
            content_type: type_from_str(&hit.project_type, query.content_type),
            downloads: hit.downloads,
            icon_url: hit.icon_url,
        })
        .collect();

    Ok(SearchResult {
        items,
        total: search_res.total_hits,
        page: query.page,
        page_size: query.page_size,
    })
}

pub async fn get_modrinth_project(
    http: &HttpClient,
    project_id: &str,
) -> Result<ContentProject, LauncherErrorPayload> {
    get_modrinth_project_with_base(http, API_BASE, project_id).await
}

pub(crate) async fn get_modrinth_project_with_base(
    http: &HttpClient,
    api_base: &str,
    project_id: &str,
) -> Result<ContentProject, LauncherErrorPayload> {
    let url = format!("{}/project/{}", api_base, project_id);
    let res = http.get(&url).await?;
    let proj: ModrinthProjectResponse = res.json().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal membaca proyek Modrinth: {}", e))
    })?;

    Ok(ContentProject {
        summary: ContentProjectSummary {
            provider: ContentProviderId::Modrinth,
            id: proj.id,
            slug: proj.slug,
            title: proj.title,
            description: proj.description,
            content_type: type_from_str(&proj.project_type, ContentType::Mod),
            downloads: proj.downloads,
            icon_url: proj.icon_url,
        },
        body: proj.body,
        updated_at: proj.updated,
        categories: proj.categories,
        loaders: proj.loaders,
        game_versions: proj.game_versions,
    })
}

pub async fn list_modrinth_versions(
    http: &HttpClient,
    project_id: &str,
    filter: &ContentFilter,
) -> Result<Vec<ModrinthVersionDetail>, LauncherErrorPayload> {
    list_modrinth_versions_with_base(http, API_BASE, project_id, filter).await
}

pub(crate) async fn list_modrinth_versions_with_base(
    http: &HttpClient,
    api_base: &str,
    project_id: &str,
    filter: &ContentFilter,
) -> Result<Vec<ModrinthVersionDetail>, LauncherErrorPayload> {
    let mut url = Url::parse(&format!("{}/project/{}/version", api_base, project_id))
        .map_err(|e| LauncherErrorPayload::internal(format!("Invalid URL: {}", e)))?;

    if let Some(ref gv) = filter.minecraft_version {
        if let Ok(json) = serde_json::to_string(&[gv]) {
            url.query_pairs_mut().append_pair("game_versions", &json);
        }
    }
    if let Some(ref l) = filter.loader {
        if let Ok(json) = serde_json::to_string(&[l]) {
            url.query_pairs_mut().append_pair("loaders", &json);
        }
    }
    if let Some(featured) = filter.featured {
        if featured {
            url.query_pairs_mut().append_pair("featured", "true");
        }
    }

    let res = http.get(url.as_str()).await?;
    let list: Vec<ModrinthVersionDetail> = res.json().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal membaca daftar versi Modrinth: {}", e))
    })?;

    Ok(list)
}

pub async fn get_modrinth_version(
    http: &HttpClient,
    version_id: &str,
) -> Result<ModrinthVersionDetail, LauncherErrorPayload> {
    get_modrinth_version_with_base(http, API_BASE, version_id).await
}

pub(crate) async fn get_modrinth_version_with_base(
    http: &HttpClient,
    api_base: &str,
    version_id: &str,
) -> Result<ModrinthVersionDetail, LauncherErrorPayload> {
    let url = format!("{}/version/{}", api_base, version_id);
    let res = http.get(&url).await?;
    let detail: ModrinthVersionDetail = res.json().await.map_err(|e| {
        LauncherErrorPayload::network(format!("Gagal membaca rincian versi Modrinth: {}", e))
    })?;

    Ok(detail)
}
