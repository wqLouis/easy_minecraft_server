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
    pub dependencies: Vec<ModrinthDependency>,
}

/// A downloadable file within a version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

/// A dependency of a specific version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthDependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: String,
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
    #[serde(alias = "followers")]
    follows: u64,
    categories: Vec<String>,
    versions: Vec<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    icon_url: Option<String>,
    client_side: Option<String>,
    server_side: Option<String>,
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
    #[serde(default)]
    dependencies: Vec<ModrinthDependency>,
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
        page_url: format!("https://modrinth.com/{}/{}", pt, slug),
        icon_url: resp.icon_url,
        latest_version_id: None,
        game_versions: resp.game_versions,
        client_side: resp.client_side,
        server_side: resp.server_side,
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
            dependencies: v.dependencies,
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

/// Fetch a single version by its Modrinth version ID.
pub async fn fetch_version_by_id(version_id: &str) -> Result<ModrinthVersion, Error> {
    let url = format!("{API_BASE}/version/{version_id}");
    let resp: VersionResponse = reqwest::get(&url).await?.json().await?;
    Ok(ModrinthVersion {
        id: resp.id,
        name: resp.name,
        version_number: resp.version_number,
        loaders: resp.loaders,
        game_versions: resp.game_versions,
        files: resp
            .files
            .into_iter()
            .map(|f| ModrinthFile {
                url: f.url,
                filename: f.filename,
                primary: f.primary,
                size: f.size,
            })
            .collect(),
        dependencies: resp.dependencies,
    })
}

/// A resolved dependency with project slug and optional version info.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedDependency {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub dependency_type: String,
    pub version_id: Option<String>,
    pub version_number: Option<String>,
    pub download_url: Option<String>,
    pub filename: Option<String>,
    pub icon_url: Option<String>,
}

/// Resolve all required dependencies for a project version matching the
/// given MC version and loader. Returns the dependency tree (non-recursive
/// for now — one level deep).
pub async fn resolve_dependencies(
    project_slug: &str,
    mc_version: &str,
    loader: &str,
) -> Result<Vec<ResolvedDependency>, Error> {
    let versions = fetch_versions(project_slug, Some(mc_version), Some(loader)).await?;
    let version = versions.first().ok_or_else(|| {
        Error::NoVersion(
            format!("modrinth/{project_slug}"),
            format!("{mc_version}+{loader}"),
        )
    })?;

    let mut resolved = Vec::new();
    for dep in &version.dependencies {
        if dep.dependency_type != "required" {
            continue;
        }
        let pid = match &dep.project_id {
            Some(id) => id.clone(),
            None => continue,
        };

        // Get project details to fetch slug and title
        let project_url = format!("{API_BASE}/project/{pid}");
        let project: ProjectResponse = match reqwest::get(&project_url).await {
            Ok(r) => match r.json().await {
                Ok(p) => p,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        // Find matching version
        let (v_id, v_num, dl_url, fname) =
            if let Some(vid) = &dep.version_id {
                // Specific version specified
                match fetch_version_by_id(vid).await {
                    Ok(v) => {
                        let file = v.files.iter().find(|f| f.primary).or_else(|| v.files.first());
                        (
                            Some(v.id.clone()),
                            Some(v.version_number.clone()),
                            file.map(|f| f.url.clone()),
                            file.map(|f| f.filename.clone()),
                        )
                    }
                    Err(_) => (None, None, None, None),
                }
            } else {
                // No specific version — find latest matching our MC+loader
                match fetch_versions(&project.slug, Some(mc_version), Some(loader)).await {
                    Ok(vs) => {
                        if let Some(v) = vs.first() {
                            let file = v.files.iter().find(|f| f.primary).or_else(|| v.files.first());
                            (
                                Some(v.id.clone()),
                                Some(v.version_number.clone()),
                                file.map(|f| f.url.clone()),
                                file.map(|f| f.filename.clone()),
                            )
                        } else {
                            (None, None, None, None)
                        }
                    }
                    Err(_) => (None, None, None, None),
                }
            };

        resolved.push(ResolvedDependency {
            project_id: pid,
            slug: project.slug,
            title: project.title,
            dependency_type: dep.dependency_type.clone(),
            version_id: v_id,
            version_number: v_num,
            download_url: dl_url,
            filename: fname,
            icon_url: project.icon_url,
        });
    }
    Ok(resolved)
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
