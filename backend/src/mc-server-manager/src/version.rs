//! Fetch available Minecraft versions from different providers.
//!
//! Thin wrapper around the [`mc-server-installer`] crate that maps
//! provider name strings to [`ServerSoftware`] variants.

use serde::Serialize;

use crate::error::Error;

/// A software provider that can be queried for versions.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub name: String,
    pub label: String,
}

/// Return the list of supported providers.
pub fn list_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            name: "vanilla".into(),
            label: "Mojang Vanilla".into(),
        },
        ProviderInfo {
            name: "paper".into(),
            label: "PaperMC".into(),
        },
        ProviderInfo {
            name: "purpur".into(),
            label: "PurpurMC".into(),
        },
        ProviderInfo {
            name: "fabric".into(),
            label: "Fabric".into(),
        },
        ProviderInfo {
            name: "forge".into(),
            label: "Forge".into(),
        },
        ProviderInfo {
            name: "neoforge".into(),
            label: "NeoForge".into(),
        },
        ProviderInfo {
            name: "waterfall".into(),
            label: "Waterfall (Proxy)".into(),
        },
        ProviderInfo {
            name: "velocity".into(),
            label: "Velocity (Proxy)".into(),
        },
    ]
}

/// Parse a provider name string into a [`mc_server_installer::ServerSoftware`].
pub fn parse_provider(name: &str) -> Result<mc_server_installer::ServerSoftware, Error> {
    match name.to_lowercase().as_str() {
        "vanilla" => Ok(mc_server_installer::ServerSoftware::Vanilla),
        "paper" => Ok(mc_server_installer::ServerSoftware::Paper),
        "purpur" => Ok(mc_server_installer::ServerSoftware::Purpur),
        "spigot" => Ok(mc_server_installer::ServerSoftware::Spigot),
        "fabric" => Ok(mc_server_installer::ServerSoftware::Fabric),
        "forge" => Ok(mc_server_installer::ServerSoftware::Forge),
        "neoforge" => Ok(mc_server_installer::ServerSoftware::NeoForge),
        "waterfall" => Ok(mc_server_installer::ServerSoftware::Waterfall),
        "velocity" => Ok(mc_server_installer::ServerSoftware::Velocity),
        _ => Err(Error::other(format!("Unknown provider '{name}'"))),
    }
}

/// Fetch available Minecraft versions for a provider.
pub async fn fetch_versions(provider: &str) -> Result<Vec<String>, Error> {
    let sw = parse_provider(provider)?;
    mc_server_installer::fetch_versions(sw)
        .await
        .map_err(|e| Error::other(format!("Failed to fetch versions: {e}")))
}

/// Fetch download info for the latest build of a specific version.
pub async fn fetch_latest(
    provider: &str,
    mc_version: &str,
) -> Result<mc_server_installer::VersionInfo, Error> {
    let sw = parse_provider(provider)?;
    mc_server_installer::fetch_latest(sw, mc_version)
        .await
        .map_err(|e| Error::other(format!("Failed to fetch version info: {e}")))
}
