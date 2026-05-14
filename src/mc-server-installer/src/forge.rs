use std::collections::HashMap;

use serde::Deserialize;

use crate::error::Error;
use crate::VersionInfo;

#[derive(Deserialize)]
struct PromosResponse {
    promos: HashMap<String, String>,
}

const PROMOS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const MAVEN_BASE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";

/// Fetch all MC versions that have a Forge release.
pub async fn fetch_versions() -> Result<Vec<String>, Error> {
    let resp: PromosResponse = reqwest::get(PROMOS_URL).await?.json().await?;

    let mut versions: Vec<String> = resp
        .promos
        .keys()
        .filter_map(|k| {
            if k.ends_with("-latest") || k.ends_with("-recommended") {
                Some(k.rsplitn(2, '-').nth(1)?.to_string())
            } else {
                None
            }
        })
        .collect();
    versions.sort();
    versions.dedup();
    versions.reverse();
    Ok(versions)
}

/// Fetch download info for the latest Forge build for a given MC version.
///
/// Forge returns an **installer JAR**. It must be run with
/// `--installServer` to produce the actual server JAR.
pub async fn fetch_latest(mc_version: &str) -> Result<VersionInfo, Error> {
    let resp: PromosResponse = reqwest::get(PROMOS_URL).await?.json().await?;

    let forge_ver = resp
        .promos
        .get(&format!("{mc_version}-latest"))
        .ok_or_else(|| Error::NoVersion("forge".into(), mc_version.into()))?;

    let download_url = format!(
        "{MAVEN_BASE}/{mc_version}-{forge_ver}/forge-{mc_version}-{forge_ver}-installer.jar"
    );

    Ok(VersionInfo {
        name: format!("Forge {mc_version}-{forge_ver}"),
        mc_version: mc_version.into(),
        build: Some(forge_ver.clone()),
        download_url,
        sha1: None,
        java_version: None,
    })
}
