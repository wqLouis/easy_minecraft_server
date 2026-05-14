use crate::error::Error;
use crate::VersionInfo;

const MAVEN_BASE: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";

/// NeoForge does not expose a simple version-list API.
/// This function returns an empty list — use `fetch_latest` with a
/// known MC version, or check the maven directory manually.
pub async fn fetch_versions() -> Result<Vec<String>, Error> {
    // NeoForge doesn't have a clean version manifest like Forge's promos.
    // For now, return an empty list. Users should specify the MC version
    // they want and we'll look up the matching NeoForge build.
    Ok(vec![])
}

/// Fetch the latest NeoForge build for a given MC version.
///
/// NeoForge returns an **installer JAR** which must be run with
/// `--install-server` to produce the actual server JAR.
///
/// This queries the Maven directory listing to find the latest version.
pub async fn fetch_latest(mc_version: &str) -> Result<VersionInfo, Error> {
    // NeoForge uses its own version scheme (e.g. 21.4.0-beta for MC 1.21.4)
    // We query the maven directory to find available versions for this MC version
    let dir_url = format!("{MAVEN_BASE}/");
    let html = reqwest::get(&dir_url).await?.text().await?;

    // Parse HTML to find version links matching our MC version
    // NeoForge versions: <a href="21.4.0-beta/">21.4.0-beta/</a>
    let mc_short = mc_version
        .strip_prefix("1.")
        .unwrap_or(mc_version);

    let matching_versions: Vec<String> = html
        .split("href=\"")
        .skip(1)
        .filter_map(|part| {
            let ver = part.split('/').next()?;
            if ver.starts_with(&mc_short) || ver.contains(mc_version) {
                Some(ver.to_string())
            } else {
                None
            }
        })
        .collect();

    let neo_ver = matching_versions
        .last()
        .ok_or_else(|| Error::NoVersion("neoforge".into(), mc_version.into()))?;

    let download_url = format!(
        "{MAVEN_BASE}/{neo_ver}/neoforge-{neo_ver}-installer.jar"
    );

    Ok(VersionInfo {
        name: format!("NeoForge {neo_ver}"),
        mc_version: mc_version.into(),
        build: Some(neo_ver.clone()),
        download_url,
        sha1: None,
        java_version: None,
    })
}
