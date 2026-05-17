use std::collections::HashMap;

use serde::Deserialize;

use crate::VersionInfo;
use crate::error::Error;

// ---------------------------------------------------------------------------
// API response models (PaperMC API v2)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ProjectResponse {
    #[allow(dead_code)]
    project_id: String,
    versions: Vec<String>,
}

#[derive(Deserialize)]
struct BuildsResponse {
    builds: Vec<BuildEntry>,
}

#[derive(Deserialize)]
struct BuildEntry {
    build: u64,
    channel: String,
    downloads: HashMap<String, DownloadEntry>,
}

#[derive(Deserialize)]
struct DownloadEntry {
    name: String,
    #[allow(dead_code)]
    sha256: Option<String>,
}

const API_BASE: &str = "https://api.papermc.io/v2";

// ---------------------------------------------------------------------------
// Version listing
// ---------------------------------------------------------------------------

/// Fetch all available Minecraft versions for a PaperMC project (v2 API).
pub async fn fetch_project_versions(project: &str) -> Result<Vec<String>, Error> {
    let url = format!("{API_BASE}/projects/{project}");
    let resp: ProjectResponse = reqwest::get(&url).await?.json().await?;
    Ok(resp.versions)
}

/// Fetch versions for Paper.
pub async fn fetch_versions() -> Result<Vec<String>, Error> {
    fetch_project_versions("paper").await
}

// ---------------------------------------------------------------------------
// Latest build info
// ---------------------------------------------------------------------------

/// Fetch the latest stable build for a PaperMC project at a given MC version.
pub async fn fetch_project_latest(project: &str, mc_version: &str) -> Result<VersionInfo, Error> {
    let url = format!("{API_BASE}/projects/{project}/versions/{mc_version}/builds");
    let resp: BuildsResponse = reqwest::get(&url).await?.json().await?;

    // Find the latest STABLE build (filter out ALPHA)
    let build = resp
        .builds
        .iter()
        .rfind(|b| b.channel == "STABLE" || b.channel == "DEFAULT")
        .or_else(|| resp.builds.last())
        .ok_or_else(|| Error::NoStableBuild(project.into(), mc_version.into()))?;

    let download = build.downloads.get("application").ok_or_else(|| {
        Error::NoVersion(
            project.into(),
            format!("{mc_version} build {}", build.build),
        )
    })?;

    let download_url = format!(
        "{API_BASE}/projects/{project}/versions/{mc_version}/builds/{}/downloads/{}",
        build.build, download.name
    );

    Ok(VersionInfo {
        name: format!("{} {} build {}", project, mc_version, build.build),
        mc_version: mc_version.into(),
        build: Some(build.build.to_string()),
        download_url,
        sha1: None, // PaperMC provides SHA-256, not SHA-1
        java_version: None,
    })
}

/// Fetch the latest Paper build.
pub async fn fetch_latest(mc_version: &str) -> Result<VersionInfo, Error> {
    fetch_project_latest("paper", mc_version).await
}
