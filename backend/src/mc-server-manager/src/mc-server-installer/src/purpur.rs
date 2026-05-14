use serde::Deserialize;

use crate::error::Error;
use crate::VersionInfo;

#[derive(Deserialize)]
struct PurpurResponse {
    #[allow(dead_code)]
    project: String,
    versions: Vec<String>,
}

#[derive(Deserialize)]
struct PurpurVersion {
    builds: BuildInfo,
}

#[derive(Deserialize)]
struct BuildInfo {
    latest: String,
    all: Vec<String>,
}

const API_BASE: &str = "https://api.purpurmc.org/v2";

/// Fetch all available Minecraft versions for Purpur.
pub async fn fetch_versions() -> Result<Vec<String>, Error> {
    let url = format!("{API_BASE}/purpur");
    let resp: PurpurResponse = reqwest::get(&url).await?.json().await?;
    Ok(resp.versions)
}

/// Fetch the latest Purpur build for a given MC version.
pub async fn fetch_latest(mc_version: &str) -> Result<VersionInfo, Error> {
    let url = format!("{API_BASE}/purpur/{mc_version}");
    let resp: PurpurVersion = reqwest::get(&url).await?.json().await?;

    let build = &resp.builds.latest;

    Ok(VersionInfo {
        name: format!("Purpur {mc_version} build {build}"),
        mc_version: mc_version.into(),
        build: Some(build.clone()),
        download_url: format!("{API_BASE}/purpur/{mc_version}/{build}/download"),
        sha1: None,
        java_version: None,
    })
}
