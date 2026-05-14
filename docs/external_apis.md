# External APIs — Minecraft Server Software Download

Programmatic APIs for downloading Minecraft server jars, mods, and plugins.
These can be used by `mc-server-manager` to fetch server software automatically.

---

## 1. PaperMC API (Paper, Waterfall, Velocity)

**Base URL:** `https://api.papermc.io/v3/`  
**Docs:** https://docs.papermc.io/misc/downloads-service  
**Auth:** None required (User-Agent header recommended)

PaperMC provides builds for **Paper**, **Waterfall** (BungeeCord fork), and **Velocity** (proxy).

### Endpoints

```
# List all projects
GET /v3/projects
→ ["paper", "waterfall", "velocity"]

# Get project metadata + available versions
GET /v3/projects/paper
→ {
    "project": "paper",
    "versions": {
      "1.21": ["1.21", "1.21.1", "1.21.2", "1.21.3", "1.21.4"],
      "1.20": ["1.20.6", ...]
    }
  }

# List builds for a specific version (filterable by channel)
GET /v3/projects/paper/versions/1.21.4/builds
→ [
    {
      "build": 138,
      "channel": "STABLE",          # "STABLE" or "EXPERIMENTAL"
      "downloads": {
        "server:default": {
          "url": "..."
        }
      }
    }
  ]

# Get latest stable build only
GET /v3/projects/paper/versions/1.21.4/builds?channel=STABLE&limit=1

# Download a specific build
GET /v3/projects/paper/versions/1.21.4/builds/138/downloads/server:default
→ 302 redirect → binary .jar download
```

### Example: download latest Paper stable

```bash
PROJECT="paper"
VERSION="1.21.4"
UA="my-app/1.0.0"

# Get latest stable build number
BUILD=$(curl -sH "User-Agent: $UA" \
  "https://api.papermc.io/v3/projects/$PROJECT/versions/$VERSION/builds?channel=STABLE&limit=1" \
  | jq -r '.[0].build')

# Download
curl -o paper-$VERSION-$BUILD.jar \
  "https://api.papermc.io/v3/projects/$PROJECT/versions/$VERSION/builds/$BUILD/downloads/server:default"
```

---

## 2. PurpurMC API

**Base URL:** `https://api.purpurmc.org/v2/`  
**Auth:** None required

Simpler API than PaperMC. Supports **Purpur** (a Paper fork with extra features).

### Endpoints

```
# List all available Minecraft versions
GET /v2/purpur
→ { "project": "purpur", "versions": ["1.14.1", ..., "1.21.4"] }

# Get latest build number for a version
GET /v2/purpur/1.21.4
→ { "builds": { "latest": 2284, "all": ["2234", "2235", ...] } }

# Download a specific build
GET /v2/purpur/1.21.4/2284/download
→ 302 redirect → .jar download
```

### Example: download latest Purpur

```bash
VERSION="1.21.4"
BUILD=$(curl -s "https://api.purpurmc.org/v2/purpur/$VERSION" | jq -r '.builds.latest')
curl -o purpur-$VERSION-$BUILD.jar \
  "https://api.purpurmc.org/v2/purpur/$VERSION/$BUILD/download"
```

---

## 3. Modrinth API

**Base URL:** `https://api.modrinth.com/v2/`  
**Docs:** https://docs.modrinth.com/api/  
**Auth:** Optional for read operations (User-Agent recommended)

Modrinth is an open-source modding platform. It hosts **mods**, **plugins**,
**datapacks**, **shaders**, and **resourcepacks** for all loaders:
Fabric, Forge, NeoForge, Quilt, Paper, Purpur, Spigot, etc.

> **Note:** Modrinth hosts *plugins* for Bukkit/Paper servers, but does **not**
> host server jar files (Paper, Purpur, etc.) directly. Use PaperMC or Purpur
> APIs for those.

### Endpoints

```
# Search for projects
GET /v2/search?query=essentials&facets=[["project_type:mod"]]&limit=10

# Get a project (by slug or ID)
GET /v2/project/essentialsx

# List versions of a project
GET /v2/project/essentialsx/version
→ [
    {
      "id": "abc123",
      "name": "EssentialsX 2.20.1",
      "version_number": "2.20.1",
      "loaders": ["paper", "spigot", "purpur"],
      "game_versions": ["1.21", "1.21.1", "1.21.2"],
      "files": [
        {
          "url": "https://cdn.modrinth.com/.../EssentialX-2.20.1.jar",
          "filename": "EssentialsX-2.20.1.jar",
          "primary": true
        }
      ]
    }
  ]

# Download the primary file of the latest version
GET /v2/project/essentialsx/version
→ parse response → download file[0].url

# Get latest version matching specific loader + game version
GET /v2/project/essentialsx/version?loaders=["paper"]&game_versions=["1.21.4"]

# Resolve a file by its hash
GET /v2/version_file/{sha1_hash}/download?algorithm=sha1
→ 302 redirect → file download
```

### Example: download latest EssentialsX for Paper 1.21

```bash
curl -s "https://api.modrinth.com/v2/project/essentialsx/version?loaders=[%22paper%22]&game_versions=[%221.21.4%22]&limit=1" \
  | jq -r '.[0].files[] | select(.primary == true) | .url' \
  | xargs curl -o essentialsx.jar
```

---

## 4. CurseForge API

**Base URL:** `https://api.curseforge.com/v1/`  
**Docs:** https://docs.curseforge.com/  
**Auth:** **API key required** (free at https://console.curseforge.com/)

CurseForge hosts mods for Forge, Fabric, and some Bukkit/Spigot plugins.

### Endpoints

```
# Search for mods
GET /v1/mods/search?gameId=432&slug=jei
Headers: x-api-key: $API_KEY

# Get files for a mod
GET /v1/mods/{modId}/files
Headers: x-api-key: $API_KEY

# Get download URL for a file
GET /v1/mods/{modId}/files/{fileId}/download-url
Headers: x-api-key: $API_KEY
```

> The API key is a **required** header on every request. Rate limits apply.

---

## 5. Spiget API (SpigotMC)

**Base URL:** `https://api.spiget.org/v2/`  
**Auth:** None (free, rate-limited)

Spiget is an unofficial API for SpigotMC.org — the largest plugin repository.

```
# Search resources
GET /v2/search/resources/{query}?size=10

# Get resource details
GET /v2/resources/{resource_id}

# Get latest download URL
GET /v2/resources/{resource_id}/download
→ 302 redirect to .jar
```

> Unofficial — may break if SpigotMC changes their site. Use Modrinth for
> critical production automation.

---

## 6. GitHub Releases

Many Minecraft projects distribute server software via GitHub Releases.
These can be fetched with the standard GitHub API.

**Base URL:** `https://api.github.com/repos/{owner}/{repo}/releases`

```
# List releases
GET /api.github.com/repos/FabricMC/fabric/releases/latest
→ { tag_name: "...", assets: [{ name: "fabric-server-mc.1.21.4.jar", browser_download_url: "..." }] }
```

Common GitHub-hosted projects:

| Project | Repo |
|---------|------|
| Fabric Server | `FabricMC/fabric` |
| Spigot BuildTools | `SpigotMC/BuildTools` |
| Pufferfish | `pufferfish-gg/Pufferfish` |

---

## Quick Reference Table

| API | Server JARs | Mods/Plugins | Auth Required | Rate Limits |
|-----|-------------|-------------|---------------|-------------|
| **PaperMC** | ✅ Paper, Waterfall, Velocity | ❌ | No | Generous |
| **PurpurMC** | ✅ Purpur | ❌ | No | Generous |
| **Modrinth** | ❌ | ✅ All loaders | No (read ops) | Yes (docs) |
| **CurseForge** | ❌ | ✅ Forge/Fabric/Bukkit | **Yes (API key)** | Yes |
| **Spiget** | ❌ | ✅ Spigot plugins | No | Yes |
| **GitHub** | ✅ Fabric installer | ✅ Various | No (unauthenticated: 60/hr) | 60/hr unauth |

---

## Integration with `mc-server-manager`

The crate currently takes a local `jar_path`. A planned enhancement is to
add a method that downloads server jars automatically:

```rust
// Future API sketch
let config = ServerConfig::new()
    .download_from(ServerSoftware::Paper, "1.21.4")
    .await?;

// This would:
//   1. Query api.papermc.io for the latest stable build of Paper 1.21.4
//   2. Download the .jar to the server directory
//   3. Set jar_path to the downloaded file
```

Supported software targets would include:
- `Paper` — via PaperMC API
- `Purpur` — via PurpurMC API
- `Waterfall` — via PaperMC API
- `Velocity` — via PaperMC API
- `Fabric` — via GitHub Releases

Plugins and mods can be fetched separately via the Modrinth API and
placed in the server's `plugins/` or `mods/` directory.
