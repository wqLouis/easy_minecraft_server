use serde::Deserialize;

use crate::error::Error;
use crate::VersionInfo;

#[derive(Deserialize)]
#[allow(dead_code)]
struct VersionManifest {
    latest: Latest,
    versions: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Latest {
    release: String,
    snapshot: String,
}

#[derive(Deserialize)]
struct ManifestEntry {
    id: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    kind: String,
    url: String,
}

#[derive(Deserialize)]
struct VersionMeta {
    downloads: Downloads,
    #[allow(dead_code)]
    java_version: Option<JavaVersion>,
}

#[derive(Deserialize)]
struct Downloads {
    server: DownloadEntry,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct DownloadEntry {
    url: String,
    sha1: String,
    size: u64,
}

#[derive(Deserialize)]
struct JavaVersion {
    #[allow(dead_code)]
    component: String,
    major_version: u32,
}

const MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Fetch all available version IDs from Mojang's manifest.
pub async fn fetch_versions() -> Result<Vec<String>, Error> {
    let manifest: VersionManifest = reqwest::get(MANIFEST_URL).await?.json().await?;
    Ok(manifest.versions.into_iter().map(|v| v.id).collect())
}

/// Fetch the latest version info for a specific Mojang version.
pub async fn fetch_latest(mc_version: &str) -> Result<VersionInfo, Error> {
    let manifest: VersionManifest = reqwest::get(MANIFEST_URL).await?.json().await?;

    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == mc_version)
        .ok_or_else(|| Error::NoVersion("vanilla".into(), mc_version.into()))?;

    let meta: VersionMeta = reqwest::get(&entry.url).await?.json().await?;

    let java_ver = meta.java_version.map(|j| j.major_version);

    Ok(VersionInfo {
        name: format!("Vanilla {}", mc_version),
        mc_version: mc_version.into(),
        build: None,
        download_url: meta.downloads.server.url,
        sha1: Some(meta.downloads.server.sha1),
        java_version: java_ver,
    })
}
