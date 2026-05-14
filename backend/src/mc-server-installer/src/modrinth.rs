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
    /// e.g. `"mod"`, `"plugin"`, `"datapack"`
    pub project_type: String,
    /// Download count
    pub downloads: u64,
    /// Compatible loaders: `["fabric", "forge"]`, `["paper", "purpur"]`, etc.
    pub loaders: Vec<String>,
    /// Compatible MC versions
    pub game_versions: Vec<String>,
    /// URL for the project page
    pub page_url: String,
    /// Latest version ID
    pub latest_version_id: Option<String>,
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
    loaders: Vec<String>,
    game_versions: Vec<String>,
    versions: Vec<String>,
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
///
/// # Arguments
/// * `query` - Search term
/// * `project_type` - Optional filter: `"mod"`, `"plugin"`, `"datapack"`, etc.
/// * `loaders` - Optional loader filter: `["paper"]`, `["fabric"]`, etc.
/// * `limit` - Max results (default 10)
pub async fn search(
    query: &str,
    project_type: Option<&str>,
    loaders: Option<&[&str]>,
    limit: u32,
) -> Result<Vec<ModrinthProject>, Error> {
    let mut facets: Vec<String> = Vec::new();

    if let Some(pt) = project_type {
        facets.push(format!("[\"project_type:{pt}\"]"));
    }
    if let Some(loaders) = loaders {
        for loader in loaders {
            facets.push(format!("[\"categories:{loader}\"]"));
        }
    }

    let facets_str = if facets.is_empty() {
        String::new()
    } else {
        format!("&facets={}", urlencoding(&format!("[{}]", facets.join(","))))
    };

    let url = format!(
        "{API_BASE}/search?query={query}{facets_str}&limit={limit}"
    );

    let resp: SearchResponse = reqwest::get(&url).await?.json().await?;

    Ok(resp
        .hits
        .into_iter()
        .map(|h| {
            let project_type = h.project_type.clone();
            let slug = h.slug.clone();
            ModrinthProject {
                slug: h.slug,
                title: h.title,
                description: h.description,
                project_type: h.project_type,
                downloads: h.downloads,
                loaders: h.loaders,
                game_versions: h.game_versions,
                page_url: format!("https://modrinth.com/{}/{}", project_type, slug),
                latest_version_id: h.versions.first().cloned(),
            }
        })
        .collect())
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
