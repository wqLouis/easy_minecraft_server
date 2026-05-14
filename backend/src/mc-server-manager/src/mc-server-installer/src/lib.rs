//! `mc-server-installer` — Download and install Minecraft server software
//! from Mojang, PaperMC, PurpurMC, Fabric, Forge, NeoForge, and fetch
//! plugins/mods from Modrinth.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use mc_server_installer::{ServerSoftware, fetch_latest, download};
//!
//! # async fn example() {
//! let info = fetch_latest(ServerSoftware::Paper, "1.21.4").await.unwrap();
//! let path = download(&info.download_url, "./server.jar").await.unwrap();
//! println!("Downloaded {} to {:?}", info.name, path);
//! # }
//! ```

pub mod error;
pub mod vanilla;
pub mod paper;
pub mod purpur;
pub mod fabric;
pub mod forge;
pub mod neoforge;
pub mod modrinth;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

// ---------------------------------------------------------------------------
// Software enumeration
// ---------------------------------------------------------------------------

/// Supported Minecraft server software types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerSoftware {
    /// Mojang vanilla server (via version manifest).
    Vanilla,
    /// PaperMC — high-performance Spigot fork.
    Paper,
    /// PurpurMC — Paper fork with extra configuration.
    Purpur,
    /// Spigot via BuildTools (compile from source — requires Git).
    Spigot,
    /// Fabric — lightweight mod loader (installer-based).
    Fabric,
    /// Forge — heavy mod loader (installer-based).
    Forge,
    /// NeoForge — community Forge fork (installer-based).
    NeoForge,
    /// Waterfall — BungeeCord fork (proxy).
    Waterfall,
    /// Velocity — modern proxy.
    Velocity,
}

impl ServerSoftware {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Paper => "paper",
            Self::Purpur => "purpur",
            Self::Spigot => "spigot",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Waterfall => "waterfall",
            Self::Velocity => "velocity",
        }
    }
}

// ---------------------------------------------------------------------------
// Version info
// ---------------------------------------------------------------------------

/// Information about a specific server build that can be downloaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Human-readable name (e.g. `"Paper 1.21.4 build 138"`).
    pub name: String,
    /// The Minecraft version (e.g. `"1.21.4"`).
    pub mc_version: String,
    /// Optional build identifier.
    pub build: Option<String>,
    /// Direct download URL for the server JAR.
    pub download_url: String,
    /// Optional SHA-1 checksum.
    pub sha1: Option<String>,
    /// Minimum Java major version required, if known.
    pub java_version: Option<u32>,
}

// ---------------------------------------------------------------------------
// Top-level API
// ---------------------------------------------------------------------------

/// Fetch the latest available Minecraft versions for a given software.
pub async fn fetch_versions(software: ServerSoftware) -> Result<Vec<String>, error::Error> {
    match software {
        ServerSoftware::Vanilla => vanilla::fetch_versions().await,
        ServerSoftware::Paper => paper::fetch_versions().await,
        ServerSoftware::Purpur => purpur::fetch_versions().await,
        ServerSoftware::Spigot => Ok(vec![]),  // BuildTools — no version list
        ServerSoftware::Fabric => fabric::fetch_versions().await,
        ServerSoftware::Forge => forge::fetch_versions().await,
        ServerSoftware::NeoForge => neoforge::fetch_versions().await,
        ServerSoftware::Waterfall => paper::fetch_project_versions("waterfall").await,
        ServerSoftware::Velocity => paper::fetch_project_versions("velocity").await,
    }
}

/// Fetch download information for the latest build of a given software
/// targeting a specific Minecraft version.
pub async fn fetch_latest(
    software: ServerSoftware,
    mc_version: &str,
) -> Result<VersionInfo, error::Error> {
    match software {
        ServerSoftware::Vanilla => vanilla::fetch_latest(mc_version).await,
        ServerSoftware::Paper => paper::fetch_latest(mc_version).await,
        ServerSoftware::Purpur => purpur::fetch_latest(mc_version).await,
        ServerSoftware::Spigot => Err(error::Error::Unsupported(
            "Spigot requires BuildTools; use `install_loader` instead".into(),
        )),
        ServerSoftware::Fabric => fabric::fetch_latest(mc_version).await,
        ServerSoftware::Forge => forge::fetch_latest(mc_version).await,
        ServerSoftware::NeoForge => neoforge::fetch_latest(mc_version).await,
        ServerSoftware::Waterfall => paper::fetch_project_latest("waterfall", mc_version).await,
        ServerSoftware::Velocity => paper::fetch_project_latest("velocity", mc_version).await,
    }
}

/// Download a file from `url` to `destination`, returning the path.
///
/// If `sha1` is provided, the download is verified after writing.
pub async fn download(
    url: &str,
    destination: impl AsRef<Path>,
) -> Result<PathBuf, error::Error> {
    let dest = destination.as_ref().to_path_buf();

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let response = reqwest::get(url).await?;
    let status = response.status();
    if !status.is_success() {
        return Err(error::Error::HttpStatus(status.as_u16(), url.into()));
    }

    let bytes = response.bytes().await?;
    let mut file = tokio::fs::File::create(&dest).await?;
    file.write_all(&bytes).await?;
    file.flush().await?;

    log::info!("Downloaded {} ({})", url, bytes.len());
    Ok(dest)
}

/// Download and verify a file against a SHA-1 checksum.
pub async fn download_verified(
    url: &str,
    destination: impl AsRef<Path>,
    sha1: &str,
) -> Result<PathBuf, error::Error> {
    let dest = destination.as_ref().to_path_buf();
    let path = download(url, &dest).await?;

    // Verify SHA-1
    let content = tokio::fs::read(&path).await?;
    let actual = sha1::Sha1::from(&content).hexdigest();

    if actual != sha1 {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(error::Error::ChecksumMismatch {
            expected: sha1.into(),
            actual,
        });
    }

    Ok(path)
}
