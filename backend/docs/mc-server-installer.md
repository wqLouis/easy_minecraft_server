# mc-server-installer — Crate API

Download and install Minecraft server software from Mojang, PaperMC,
PurpurMC, Fabric, Forge, NeoForge, and fetch plugins/mods from Modrinth.

**Location:** `backend/src/mc-server-installer/`

---

## Quick start

```rust
use mc_server_installer::{ServerSoftware, fetch_latest, download};

// Fetch the latest Paper build for 1.21.4
let info = fetch_latest(ServerSoftware::Paper, "1.21.4").await.unwrap();
println!("{}", info.name);        // "Paper 1.21.4 build 138"
println!("{}", info.download_url); // download URL

// Download it
let path = download(&info.download_url, "./paper.jar").await.unwrap();
```

## Fetch available Minecraft versions

```rust
let versions = mc_server_installer::fetch_versions(ServerSoftware::Paper).await?;
// → ["1.21.4", "1.21.3", "1.21.2", ...]
```

## Fetch latest version info

```rust
let info = fetch_latest(ServerSoftware::Paper, "1.21.4").await?;
```

Returns a `VersionInfo` struct:

```rust
pub struct VersionInfo {
    pub name: String,             // "Paper 1.21.4 build 138"
    pub mc_version: String,       // "1.21.4"
    pub build: Option<String>,    // Some("138")
    pub download_url: String,     // direct .jar URL
    pub sha1: Option<String>,     // SHA-1 checksum (Some for Vanilla)
    pub java_version: Option<u32>,// Java requirement (Some for Vanilla)
}
```

## Download

```rust
// Simple download
let path = download(&info.download_url, "./server.jar").await?;

// Download + SHA-1 verification (only Vanilla provides sha1)
if let Some(sha1) = &info.sha1 {
    let path = download_verified(&info.download_url, "./server.jar", sha1).await?;
}
```

## Supported software

| Software | `ServerSoftware` enum | Downloaded artifact |
|----------|----------------------|---------------------|
| **Vanilla** | `Vanilla` | Direct server JAR |
| **Paper** | `Paper` | Direct server JAR |
| **Purpur** | `Purpur` | Direct server JAR |
| **Spigot** | `Spigot` | ❌ requires BuildTools (run manually) |
| **Fabric** | `Fabric` | Installer JAR (must run with `java -jar installer.jar server ...`) |
| **Forge** | `Forge` | Installer JAR (must run with `--installServer`) |
| **NeoForge** | `NeoForge` | Installer JAR (must run with `--install-server`) |
| **Waterfall** | `Waterfall` | Direct JAR (proxy) |
| **Velocity** | `Velocity` | Direct JAR (proxy) |

### Installer-based platforms (Fabric, Forge, NeoForge)

These platforms require running an installer JAR after downloading. Example:

```rust
use mc_server_installer::{ServerSoftware, fetch_latest, download};
use std::process::Stdio;

// Download the Fabric installer
let info = fetch_latest(ServerSoftware::Fabric, "1.21.4").await?;
let installer_path = download(&info.download_url, "./fabric-installer.jar").await?;

// Run the installer to produce fabric-server-launch.jar
tokio::process::Command::new("java")
    .args([
        "-jar", "fabric-installer.jar",
        "server",
        "-mcversion", "1.21.4",
        "-loader", info.build.as_ref().unwrap(),
        "-downloadMinecraft",
    ])
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()?
    .wait()
    .await?;
```

## Modrinth — search and download plugins/mods

```rust
use mc_server_installer::modrinth;

// Search for plugins
let results = modrinth::search(
    "EssentialsX",
    Some("plugin"),           // project type filter
    Some(&["paper"]),         // loader filter
    5,                        // max results
).await?;

for project in &results {
    println!("{} — {} ({} downloads)", project.title, project.description, project.downloads);
}

// Get download URL for the latest version matching a specific MC version + loader
let url = modrinth::get_download_url("essentialsx", "1.21.4", "paper").await?;
download(&url, "./plugins/essentialsx.jar").await?;
```

## Module reference

| Module | Description |
|--------|-------------|
| `vanilla` | Mojang version manifest API |
| `paper` | PaperMC API v3 (Paper, Waterfall, Velocity) |
| `purpur` | PurpurMC API v2 |
| `fabric` | Fabric Meta API v2 |
| `forge` | Forge Promotions JSON + Maven |
| `neoforge` | NeoForge Maven (HTML directory parsing) |
| `modrinth` | Modrinth API v2 — search, version lookup, download |

## Dependencies

- `reqwest` — HTTP client
- `serde` / `serde_json` — API response parsing
- `sha1` — SHA-1 checksum verification
- `tokio` — async I/O
- `thiserror` — error types
