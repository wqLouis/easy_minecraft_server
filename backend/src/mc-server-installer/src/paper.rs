use std::collections::HashMap;

use serde::Deserialize;

use crate::error::Error;
use crate::VersionInfo;

#[derive(Deserialize)]
struct ProjectResponse {
    project: String,
    versions: HashMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct BuildsResponse(Vec<BuildEntry>);

#[derive(Deserialize)]
struct BuildEntry {
    build: u64,
    channel: String,
    downloads: HashMap<String, DownloadEntry>,
}

#[derive(Deserialize)]
struct DownloadEntry {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    sha256: Option<String>,
    url: Option<String>,
}

const API_BASE: &str = "https://api.papermc.io/v3";

/// Fetch all available Minecraft versions for a PaperMC project.
pub async fn fetch_project_versions(project: &str) -> Result<Vec<String>, Error> {
    let url = format!("{API_BASE}/projects/{project}");
    let resp: ProjectResponse = reqwest::get(&url).await?.json().await?;
    // Flatten version groups into a sorted list
    let mut versions: Vec<String> = resp
        .versions
        .into_values()
        .flat_map(|v| v)
        .collect();
    // API returns versions grouped by major version, newest first within each group
    versions.reverse();
    Ok(versions)
}

/// Fetch versions for Paper.
pub async fn fetch_versions() -> Result<Vec<String>, Error> {
    fetch_project_versions("paper").await
}

/// Fetch the latest stable build for a PaperMC project at a given MC version.
pub async fn fetch_project_latest(
    project: &str,
    mc_version: &str,
) -> Result<VersionInfo, Error> {
    let url = format!(
        "{API_BASE}/projects/{project}/versions/{mc_version}/builds?channel=STABLE&limit=1"
    );
    let resp: BuildsResponse = reqwest::get(&url).await?.json().await?;

    let build = resp.0.first().ok_or_else(|| {
        Error::NoStableBuild(project.into(), mc_version.into())
    })?;

    let download = build.downloads.get("server:default").ok_or_else(|| {
        Error::NoVersion(project.into(), format!("{mc_version} build {}", build.build))
    })?;

    let download_url = download.url.clone().unwrap_or_else(|| {
        format!(
            "{API_BASE}/projects/{project}/versions/{mc_version}/builds/{}/downloads/server:default",
            build.build
        )
    });

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
