use serde::{Deserialize, Serialize};

use crate::error::Error;

const API_BASE: &str = "https://api.modrinth.com/v2";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A project (mod/plugin) on Modrinth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthProject {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub project_type: String,
    pub downloads: u64,
    pub follows: u64,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub page_url: String,
    pub icon_url: Option<String>,
    pub latest_version_id: Option<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
    pub color: Option<u32>,
}

/// A specific version file of a Modrinth project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub files: Vec<ModrinthFile>,
}

/// A downloadable file within a version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

// ---------------------------------------------------------------------------
// Internal API types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    slug: String,
    title: String,
    description: String,
    project_type: String,
    downloads: u64,
    follows: u64,
    categories: Vec<String>,
    versions: Vec<String>,
    icon_url: Option<String>,
    latest_version: Option<String>,
    client_side: Option<String>,
    server_side: Option<String>,
    color: Option<u32>,
}

#[derive(Deserialize)]
struct ProjectResponse {
    slug: String,
    title: String,
    description: String,
    project_type: String,
    downloads: u64,
    follows: u64,
    categories: Vec<String>,
    versions: Vec<String>,
    icon_url: Option<String>,
    client_side: String,
    server_side: String,
    color: Option<u32>,
    license: serde_json::Value,
}

#[derive(Deserialize)]
struct VersionResponse {
    id: String,
    name: String,
    version_number: String,
    loaders: Vec<String>,
    game_versions: Vec<String>,
    files: Vec<VersionFile>,
}

#[derive(Deserialize)]
struct VersionFile {
    url: String,
    filename: String,
    primary: bool,
    size: u64,
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Search for projects on Modrinth.
pub async fn search(
    query: &str,
    project_type: Option<&str>,
    loaders: Option<&[&str]>,
    versions: Option<&[&str]>,
    client_side: Option<&str>,
    server_side: Option<&str>,
    open_source: Option<bool>,
    index: Option<&str>,
    offset: u32,
    limit: u32,
) -> Result<Vec<ModrinthProject>, Error> {
    let mut facets: Vec<String> = Vec::new();

    if let Some(pt) = project_type {
        facets.push(format!("[\"project_type:{pt}\"]"));
    }
    if let Some(ls) = loaders {
        for l in ls {
            facets.push(format!("[\"categories:{l}\"]"));
        }
    }
    if let Some(vs) = versions {
        for v in vs {
            facets.push(format!("[\"versions:{v}\"]"));
        }
    }
    if let Some(cs) = client_side {
        facets.push(format!("[\"client_side:{cs}\"]"));
    }
    if let Some(ss) = server_side {
        facets.push(format!("[\"server_side:{ss}\"]"));
    }
    if let Some(os) = open_source {
        facets.push(format!("[\"open_source:{os}\"]"));
    }

    let facets_str = if facets.is_empty() {
        String::new()
    } else {
        format!(
            "&facets={}",
            urlencoding(&format!("[{}]", facets.join(",")))
        )
    };
    let idx = index.map(|i| format!("&index={i}")).unwrap_or_default();
    let off = if offset > 0 {
        format!("&offset={offset}")
    } else {
        String::new()
    };
    let url = format!("{API_BASE}/search?query={query}{facets_str}{idx}{off}&limit={limit}");

    let resp: SearchResponse = reqwest::get(&url).await?.json().await?;

    Ok(resp
        .hits
        .into_iter()
        .map(|h| {
            let pt = h.project_type.clone();
            let slug = h.slug.clone();
            ModrinthProject {
                slug: h.slug,
                title: h.title,
                description: h.description,
                project_type: h.project_type,
                downloads: h.downloads,
                follows: h.follows,
                loaders: h.categories,
                game_versions: h.versions,
                page_url: format!("https://modrinth.com/{}/{}", pt, slug),
                icon_url: h.icon_url,
                latest_version_id: h.latest_version,
                client_side: h.client_side,
                server_side: h.server_side,
                color: h.color,
            }
        })
        .collect())
}

/// Fetch full project details by slug.
pub async fn get_project(project_slug: &str) -> Result<ModrinthProject, Error> {
    let url = format!("{API_BASE}/project/{project_slug}");
    let resp: ProjectResponse = reqwest::get(&url).await?.json().await?;
    let pt = resp.project_type.clone();
    let slug = resp.slug.clone();
    Ok(ModrinthProject {
        slug: resp.slug,
        title: resp.title,
        description: resp.description,
        project_type: resp.project_type,
        downloads: resp.downloads,
        follows: resp.follows,
        loaders: resp.categories,
        game_versions: resp.versions,
        page_url: format!("https://modrinth.com/{}/{}", pt, slug),
        icon_url: resp.icon_url,
        latest_version_id: None,
        client_side: Some(resp.client_side),
        server_side: Some(resp.server_side),
        color: resp.color,
    })
}

/// Fetch all versions of a project, optionally filtered by MC version and loader.
pub async fn fetch_versions(
    project_slug: &str,
    mc_version: Option<&str>,
    loader: Option<&str>,
) -> Result<Vec<ModrinthVersion>, Error> {
    let mut url = format!("{API_BASE}/project/{project_slug}/version");

    let mut params: Vec<String> = Vec::new();
    if let Some(mc) = mc_version {
        params.push(format!("game_versions=[\"{mc}\"]"));
    }
    if let Some(l) = loader {
        params.push(format!("loaders=[\"{l}\"]"));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let resp: Vec<VersionResponse> = reqwest::get(&url).await?.json().await?;

    Ok(resp
        .into_iter()
        .map(|v| ModrinthVersion {
            id: v.id,
            name: v.name,
            version_number: v.version_number,
            loaders: v.loaders,
            game_versions: v.game_versions,
            files: v
                .files
                .into_iter()
                .map(|f| ModrinthFile {
                    url: f.url,
                    filename: f.filename,
                    primary: f.primary,
                    size: f.size,
                })
                .collect(),
        })
        .collect())
}

/// Get the primary download URL for the latest version of a project
/// matching the given MC version and loader.
pub async fn get_download_url(
    project_slug: &str,
    mc_version: &str,
    loader: &str,
) -> Result<String, Error> {
    let versions = fetch_versions(project_slug, Some(mc_version), Some(loader)).await?;

    let version = versions.first().ok_or_else(|| {
        Error::NoVersion(
            format!("modrinth/{project_slug}"),
            format!("{mc_version}+{loader}"),
        )
    })?;

    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            Error::NoVersion(
                format!("modrinth/{project_slug}/files"),
                version.version_number.clone(),
            )
        })?;

    Ok(file.url.clone())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn urlencoding(input: &str) -> String {
    // Simple URL-encoding for the characters Modrinth needs
    let mut output = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '[' => output.push_str("%5B"),
            ']' => output.push_str("%5D"),
            '"' => output.push_str("%22"),
            ',' => output.push_str("%2C"),
            ' ' => output.push_str("%20"),
            _ => output.push(c),
        }
    }
    output
}
