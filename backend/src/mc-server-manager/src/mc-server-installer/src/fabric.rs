use serde::Deserialize;

use crate::error::Error;
use crate::VersionInfo;

#[derive(Deserialize)]
struct GameVersions(Vec<GameEntry>);

#[derive(Deserialize)]
struct GameEntry {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
struct LoaderVersions(Vec<LoaderEntry>);

#[derive(Deserialize)]
struct LoaderEntry {
    loader: LoaderInfo,
}

#[derive(Deserialize)]
struct LoaderInfo {
    version: String,
}

#[derive(Deserialize)]
struct InstallerVersions(Vec<InstallerEntry>);

#[derive(Deserialize)]
struct InstallerEntry {
    version: String,
    url: String,
    stable: bool,
}

const META_BASE: &str = "https://meta.fabricmc.net/v2";

/// Fetch all MC versions that Fabric supports.
pub async fn fetch_versions() -> Result<Vec<String>, Error> {
    let resp: GameVersions = reqwest::get(format!("{META_BASE}/versions/game"))
        .await?
        .json()
        .await?;
    Ok(resp.0.into_iter().map(|g| g.version).collect())
}

/// Get the latest Fabric loader version for a given MC version.
///
/// Fabric does NOT provide a pre-built server JAR. The returned
/// `VersionInfo` points to the **installer JAR** which must be run
/// to produce a `fabric-server-launch.jar`.
pub async fn fetch_latest(mc_version: &str) -> Result<VersionInfo, Error> {
    // Get latest loader version for this MC version
    let loaders: LoaderVersions = reqwest::get(format!(
        "{META_BASE}/versions/loader/{mc_version}"
    ))
    .await?
    .json()
    .await?;

    let loader = loaders
        .0
        .first()
        .ok_or_else(|| Error::NoVersion("fabric-loader".into(), mc_version.into()))?;

    let loader_ver = &loader.loader.version;

    // Get latest installer
    let installers: InstallerVersions = reqwest::get(format!("{META_BASE}/versions/installer"))
        .await?
        .json()
        .await?;

    let installer = installers
        .0
        .iter()
        .find(|i| i.stable)
        .or_else(|| installers.0.first())
        .ok_or_else(|| Error::NoVersion("fabric-installer".into(), "latest".into()))?;

    Ok(VersionInfo {
        name: format!("Fabric {mc_version} loader {loader_ver}"),
        mc_version: mc_version.into(),
        build: Some(loader_ver.clone()),
        download_url: installer.url.clone(),
        sha1: None,
        java_version: None,
    })
}
