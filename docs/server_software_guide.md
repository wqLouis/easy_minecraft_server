# Server Software Installation Guide

How to obtain and install every major Minecraft server platform, from
vanilla Mojang to modded platforms like Fabric, Forge, and NeoForge.

---

## Table of Contents

1. [Mojang (Vanilla) Server](#1-mojang-vanilla-server)
2. [Paper](#2-paper)
3. [Purpur](#3-purpur)
4. [Spigot / CraftBukkit](#4-spigot--craftbukkit)
5. [Fabric](#5-fabric)
6. [Forge](#6-forge)
7. [NeoForge](#7-neoforge)
8. [Comparison Table](#8-comparison-table)

---

## 1. Mojang (Vanilla) Server

**Source:** Official Mojang metadata API  
**Type:** Direct `.jar` download, runnable as-is

### API

Mojang publishes a version manifest at:

```
GET https://piston-meta.mojang.com/mc/game/version_manifest_v2.json
```

This returns all available versions (release + snapshot) with their metadata
URLs. Each metadata file contains download URLs for `client.jar` and
`server.jar`.

### Fetch the latest server.jar

```bash
# 1. Get the version manifest
MANIFEST=$(curl -s https://piston-meta.mojang.com/mc/game/version_manifest_v2.json)

# 2. Extract the latest release version URL
VERSION_URL=$(echo "$MANIFEST" | jq -r '.versions[] | select(.id == "1.21.4") | .url')
# Or use the latest release:
# LATEST=$(echo "$MANIFEST" | jq -r '.latest.release')
# VERSION_URL=$(echo "$MANIFEST" | jq -r ".versions[] | select(.id == \"$LATEST\") | .url")

# 3. Get the server download URL
SERVER_URL=$(curl -s "$VERSION_URL" | jq -r '.downloads.server.url')

# 4. Download
curl -o server.jar "$SERVER_URL"
```

### Version manifest structure

```
version_manifest_v2.json
├── latest
│   ├── release: "1.21.4"
│   └── snapshot: "26.2-snapshot-7"
└── versions[]
    └── {
          id: "1.21.4",
          type: "release",
          url: "https://piston-meta.mojang.com/.../1.21.4.json"
        }

1.21.4.json
├── downloads
│   ├── client: { url, sha1, size }
│   └── server: { url, sha1, size }
├── javaVersion: { component, majorVersion }
└── ...
```

### Notes

- The `server.jar` is a direct download — no installer needed.
- `javaVersion.majorVersion` tells you the minimum Java version required
  (e.g. 21 for modern versions, 17 for earlier).
- SHA-1 checksums are provided for verification.

---

## 2. Paper

**Source:** PaperMC API (`api.papermc.io/v3/`)  
**Type:** Pre-built `.jar`, runnable as-is  
**API docs:** https://docs.papermc.io/misc/downloads-service

Paper is a high-performance fork of Spigot. It is the most popular
Minecraft server platform and the recommended default for most servers.

### Fetch the latest stable build

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

### Run

```bash
java -jar paper-1.21.4-138.jar nogui
```

### Notes

- Pre-built, no compilation or installer needed.
- Channels: `STABLE` (production-safe) and `EXPERIMENTAL` (cutting-edge).
- Also available via API: **Waterfall** (BungeeCord proxy fork) and
  **Velocity** (modern proxy) using the same API pattern.

---

## 3. Purpur

**Source:** PurpurMC API (`api.purpurmc.org/v2/`)  
**Type:** Pre-built `.jar`, runnable as-is

Purpur is a fork of Paper (via Pufferfish) with hundreds of extra
configurable gameplay options.

### Fetch the latest build

```bash
VERSION="1.21.4"
BUILD=$(curl -s "https://api.purpurmc.org/v2/purpur/$VERSION" | jq -r '.builds.latest')
curl -o purpur-$VERSION-$BUILD.jar \
  "https://api.purpurmc.org/v2/purpur/$VERSION/$BUILD/download"
```

### Run

```bash
java -jar purpur-1.21.4-2284.jar nogui
```

### Notes

- Same API pattern as Paper but simpler (no channels, no v3 complexity).
- Supports all Paper/Spigot/Bukkit plugins.
- Extra config in `purpur.yml`.

---

## 4. Spigot / CraftBukkit

**Source:** BuildTools (`hub.spigotmc.org`)  
**Type:** Must be **compiled from source** — no pre-built JARs

Spigot and CraftBukkit are **not** distributed as pre-compiled JARs due
to legal restrictions. You must run BuildTools, which downloads Mojang's
mapped source code (via Minecraft Coder Pack) and compiles it.

### Build

```bash
# Download BuildTools
curl -o BuildTools.jar \
  "https://hub.spigotmc.org/jenkins/job/BuildTools/lastSuccessfulBuild/artifact/target/BuildTools.jar"

# Build Spigot for a specific version (requires Git)
java -jar BuildTools.jar --rev 1.21.4

# Output:
#   spigot-1.21.4.jar        ← the server jar
#   craftbukkit-1.21.4.jar   ← also produced
#   Bukkit/                   ← compiled API source
```

### Run

```bash
java -jar spigot-1.21.4.jar nogui
```

### Notes

- **Requires Git** installed on the system.
- BuildTools compiles from Mojang's obfuscated source using MCP
  (Minecraft Coder Pack) mappings.
- First build can take **5–15 minutes** depending on hardware.
- Subsequent builds are faster (cached in `~/.m2/`).
- This is why **Paper is preferred** — it provides pre-built downloads
  and has better performance.

---

## 5. Fabric

**Source:** Fabric Meta API (`meta.fabricmc.net/v2/`)  
**Type:** Installer-based — combines Mojang server + Fabric loader

Fabric is a lightweight modding framework. Unlike Paper, it does **not**
provide a pre-built server JAR. Instead, a special launcher JAR is
produced that loads the Fabric loader at runtime.

### Architecture

```
Mojang server.jar  +  Fabric Loader  →  fabric-server-launch.jar
                          ↑
                   (intermediary mappings)
```

The server is launched via `fabric-server-launch.jar`, which:
1. Downloads the vanilla Mojang server jar (if not present)
2. Patches it with Fabric intermediary mappings
3. Loads Fabric mods from the `mods/` folder

### Install

```bash
# 1. Get the latest installer version
INSTALLER_VERSION=$(curl -s https://meta.fabricmc.net/v2/versions/installer \
  | jq -r '.[0].version')

# 2. Download the installer
curl -o fabric-installer.jar \
  "https://maven.fabricmc.net/net/fabricmc/fabric-installer/$INSTALLER_VERSION/fabric-installer-$INSTALLER_VERSION.jar"

# 3. Install the server (downloads Mojang jar + applies loader)
java -jar fabric-installer.jar server \
  -mcversion 1.21.4 \
  -loader 0.19.2 \
  -downloadMinecraft

# Output:
#   fabric-server-launch.jar   ← the runnable server jar
#   libraries/                 ← downloaded dependencies
#   versions/                  ← vanilla Mojang jars cached here
```

### Run

```bash
java -jar fabric-server-launch.jar nogui
```

### API — get loader versions for a MC version

```bash
curl -s "https://meta.fabricmc.net/v2/versions/loader/1.21.4" \
  | jq -r '.[0].loader.version'
# → "0.19.2"
```

### API — get the server launch metadata (JSON)

```bash
curl -s "https://meta.fabricmc.net/v2/versions/loader/1.21.4/0.19.2/server/json"
```

This returns a JSON object with the full library list, main class, and
JVM arguments — useful for advanced integrations.

### Notes

- The Fabric **API mod** must also be installed in the `mods/` folder
  for most Fabric mods to work. Download it from Modrinth or CurseForge.
- Fabric preserves vanilla game mechanics better than Paper (no
  performance patches unless you add mods like Lithium).
- Performance mods: **Lithium** (general), **Phosphor** (lighting),
  **Starlight** (lighting rewrite), **Krypton** (networking).

---

## 6. Forge

**Source:** Forge Promotions API + Maven  
**Type:** Installer-based — patches Mojang server

Forge is the oldest and largest modding framework. It patches the Mojang
server jar directly and produces a combined server jar.

### API — find the latest Forge version

```bash
curl -s "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json" \
  | jq -r '.promos["1.21.4-latest"]'
# → "49.0.1"
```

The promos JSON maps `{mc_version}-latest` and `{mc_version}-recommended`
to Forge build numbers.

### Install

```bash
MC_VERSION="1.21.4"
FORGE_VERSION=$(curl -s "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json" \
  | jq -r ".promos[\"$MC_VERSION-latest\"]")

# Download the installer
curl -o forge-installer.jar \
  "https://maven.minecraftforge.net/net/minecraftforge/forge/$MC_VERSION-$FORGE_VERSION/forge-$MC_VERSION-$FORGE_VERSION-installer.jar"

# Run the installer
java -jar forge-installer.jar --installServer

# Output:
#   forge-$MC_VERSION-$FORGE_VERSION-server.jar   ← the runnable server jar
#   libraries/                                    ← dependencies
#   run.bat / run.sh                              ← launcher scripts
```

### Run

```bash
java -jar forge-1.21.4-49.0.1-server.jar nogui
```

### Notes

- The `--installServer` flag runs the installer in headless mode.
- Forge downloads the vanilla Mojang server jar automatically.
- Mods go in the `mods/` folder. Large modpacks can have 100+ mods.
- Forge is heavier than Fabric — expect slower startup and more memory
  usage with large modpacks.

---

## 7. NeoForge

**Source:** NeoForge Maven (`maven.neoforged.net`)  
**Type:** Installer-based — fork of Forge

NeoForge is a community-driven fork of Forge created after the Forge
project leadership controversy. It follows a faster release cycle and
supports modern Minecraft versions sooner.

### API — find the latest version

NeoForge does not have a simple promos JSON like Forge. Check the
maven repository for available versions:

```bash
# List available versions for a MC version
curl -s "https://maven.neoforged.net/releases/net/neoforged/neoforge/" \
  | grep -o 'href="[^"]*/"' | tr -d '/'
```

Or use the NeoForge API:

```
GET https://api.neoforged.net/api/v1/minecraft/{mc_version}
```

### Install

```bash
MC_VERSION="1.21.4"
NEO_VERSION="21.4.0-beta"   # from maven listing

# Download the installer
curl -o neoforge-installer.jar \
  "https://maven.neoforged.net/releases/net/neoforged/neoforge/$NEO_VERSION/neoforge-$NEO_VERSION-installer.jar"

# Run the installer
java -jar neoforge-installer.jar --install-server

# Output:
#   neoforge-$NEO_VERSION-server.jar   ← the runnable server jar
```

### Run

```bash
java -jar neoforge-21.4.0-beta-server.jar nogui
```

### Notes

- API and Maven repository are still evolving — check
  https://docs.neoforged.net/ for the latest docs.
- Supports all Forge mods (most of the time) via the `mods/` folder.
- Faster update cycle than classic Forge.

---

## 8. Comparison Table

| Platform | Download method | Pre-built? | Installer needed? | Plugin/mod API | Performance |
|----------|----------------|------------|-------------------|---------------|-------------|
| **Vanilla** | Mojang manifest | ✅ Yes | ❌ No | None | Baseline |
| **Paper** | PaperMC API | ✅ Yes | ❌ No | Bukkit/Spigot/Paper | ★★★★★ |
| **Purpur** | PurpurMC API | ✅ Yes | ❌ No | Bukkit/Spigot/Paper/Purpur | ★★★★★ |
| **Spigot** | BuildTools | ❌ Compile from source | ❌ No (but compilation required) | Bukkit/Spigot | ★★★☆☆ |
| **Fabric** | Fabric Meta API | ❌ Assembled via installer | ✅ `fabric-installer.jar` | Fabric mods | ★★★★☆ (with Lithium) |
| **Forge** | Forge Maven | ❌ Patched via installer | ✅ `forge-installer.jar --installServer` | Forge mods | ★★☆☆☆ |
| **NeoForge** | NeoForge Maven | ❌ Patched via installer | ✅ `neoforge-installer.jar --install-server` | Forge/NeoForge mods | ★★☆☆☆ |
| **Waterfall** | PaperMC API | ✅ Yes | ❌ No | BungeeCord plugins | Proxy |
| **Velocity** | PaperMC API | ✅ Yes | ❌ No | Velocity plugins | Proxy |

### Plugin/mod compatibility

```
Bukkit/Spigot plugins  →  Paper, Purpur, Spigot
Paper API plugins      →  Paper, Purpur
Fabric mods             →  Fabric (only)
Forge mods              →  Forge, NeoForge (most)
```

Choose your platform based on what you want to run:
- **Plugins** (EssentialsX, WorldEdit, LuckPerms) → **Paper**
- **Light mods** (performance, vanilla+) → **Fabric**
- **Heavy modpacks** (FTB, All the Mods) → **Forge / NeoForge**
- **Extra gameplay tweaks** → **Purpur**
- **Proxy / multi-server** → **Velocity**
