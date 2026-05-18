# Backend API

All endpoints require `Authorization: Bearer <api_key>`.  
Endpoints marked **Sudo** additionally require a sudo user.

## Authentication / Admin

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/api/health` | `GET` | Auth | Health check — returns `"ok"` |
| `/api/auth/me` | `GET` | Auth | Returns the authenticated user's profile |
| `/api/auth/register` | `POST` | Sudo | Create a new *non-sudo* user; returns API key once |
| `/api/users` | `GET` | Sudo | List all users |
| `/api/users/{id}` | `DELETE` | Sudo | Delete a user (cannot delete yourself) |
| `/api/users/{id}` | `PUT` | Sudo | Update a user's username |
| `/api/settings` | `GET` | Sudo | Returns backend settings |
| `/api/settings` | `PUT` | Sudo | Updates backend settings |
| `/api/settings/schema` | `GET` | Sudo | Returns JSON Schema for settings |
| `/api/ipban/status` | `GET` | Sudo | Returns blacklisted IPs and config |
| `/api/ipban/blacklist` | `POST` | Sudo | Add an IP to the blacklist |
| `/api/ipban/unban` | `POST` | Sudo | Remove an IP from the blacklist |

## Server Versions (Sudo)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/providers` | `GET` | List supported software providers |
| `/api/providers/{provider}/versions` | `GET` | List available MC versions |
| `/api/providers/{provider}/versions/{version}` | `GET` | Get download info for a specific build |

**Providers:** `vanilla`, `paper`, `purpur`, `fabric`, `forge`, `neoforge`, `waterfall`, `velocity`

Version info response:
```json
{
  "name": "paper 1.21.4 build 232",
  "mc_version": "1.21.4",
  "build": "232",
  "download_url": "https://api.papermc.io/v2/projects/paper/versions/1.21.4/builds/232/downloads/paper-1.21.4-232.jar",
  "sha1": null,
  "java_version": null
}
```

## Server Instances (Sudo)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/instances` | `GET` | List all instances |
| `/api/instances` | `POST` | Create a new instance |
| `/api/instances/{id}` | `GET` | Get instance details (config + status) |
| `/api/instances/{id}` | `DELETE` | Remove an instance |
| `/api/instances/{id}/config` | `PUT` | Update instance config (requires restart) |
| `/api/instances/{id}/start` | `POST` | Start the server |
| `/api/instances/{id}/stop` | `POST` | Stop the server |
| `/api/instances/{id}/command` | `POST` | Send a console command |
| `/api/instances/{id}/logs?tail=N` | `GET` | Get recent log lines (default 100) |
| `/api/instances/{id}/players` | `GET` | Get online players |
| `/api/instances/{id}/logs/stream` | `GET` | SSE stream of live server logs |
| `/api/instances/{id}/properties` | `GET` | Get all server.properties |
| `/api/instances/{id}/properties` | `PUT` | Update server.properties |
| `/api/instances/{id}/command/history` | `GET` | Get recent console command history |
| `/api/instances/schema` | `GET` | Get JSON schema for instance config |
| `/api/instances/archived` | `GET` | List archived (deleted) instances |
| `/api/instances/archived/{id}/restore` | `POST` | Restore an archived instance |

### World Management (Sudo)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/instances/{id}/worlds` | `GET` | List worlds in the server directory |
| `/api/instances/{id}/worlds/backup` | `POST` | Backup selected worlds as a zip |
| `/api/instances/{id}/worlds/upload` | `POST` | Upload a world zip to extract |
| `/api/instances/{id}/worlds/{world_name}` | `DELETE` | Delete a world directory |
| `/api/instances/{id}/worlds/{world_name}/download` | `GET` | Download a world as zip |
| `/api/instances/{id}/worlds/reset` | `POST` | Reset all worlds to fresh state |
| `/api/instances/{id}/backups` | `GET` | List available world backups |

### Mods / Plugins (Sudo)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/modrinth/search` | `GET` | Search Modrinth for mods/plugins/datapacks |
| `/api/modrinth/project/{slug}` | `GET` | Get project details from Modrinth |
| `/api/modrinth/project/{slug}/versions` | `GET` | List versions for a Modrinth project |
| `/api/modrinth/project/{slug}/download-url` | `GET` | Get direct download URL for a version |
| `/api/instances/{id}/mods` | `GET` | List installed mods/plugins |
| `/api/instances/{id}/mods/install` | `POST` | Install a mod/plugin from a direct URL |
| `/api/instances/{id}/mods/{filename}` | `DELETE` | Delete a mod/plugin (server must be stopped) |
| `/api/instances/{id}/mods/{filename}/toggle` | `PUT` | Enable or disable a mod/plugin |
| `/api/instances/{id}/mods/modpack` | `POST` | Generate a Modrinth-format modpack (.mrpack) |
| `/api/instances/{id}/mods/modpack/download` | `GET` | Download the generated modpack file |

### Modrinth Search Query Parameters

| Param | Type | Description |
|-------|------|-------------|
| `query` | `string` | Search query (required) |
| `type` | `string` | Project type filter (`mod`, `plugin`, `datapack`, `modpack`, `shader`) |
| `loaders` | `string` | Comma-separated loader filter (e.g. `fabric,paper`) |
| `versions` | `string` | Comma-separated MC version filter (e.g. `1.21.4,1.21.3`) |
| `client_side` | `string` | Client-side support (`required`, `optional`, `unsupported`) |
| `server_side` | `string` | Server-side support (`required`, `optional`, `unsupported`) |
| `open_source` | `bool` | Filter by open source |
| `index` | `string` | Sort order (`relevance`, `downloads`, `follows`, `newest`, `updated`) |
| `offset` | `int` | Pagination offset |
| `limit` | `int` | Max results (default 10, max 100) |

### Modrinth Search Response

```json
{
  "results": [
    {
      "slug": "essentialsx",
      "title": "EssentialsX",
      "description": "The essential plugin...",
      "project_type": "plugin",
      "downloads": 12345678,
      "loaders": ["paper", "purpur", "spigot"],
      "game_versions": ["1.21.4", "1.21.3"],
      "page_url": "https://modrinth.com/plugin/essentialsx",
      "icon_url": "https://cdn.modrinth.com/..."
    }
  ],
  "total_hits": 42
}
```

### Install Mod Request / Response

**Request:** `POST /api/instances/{id}/mods/install`
```json
{
  "download_url": "https://cdn.modrinth.com/.../essentialx.jar",
  "filename": "essentialsx.jar"
}
```

**Response:**
```json
{
  "installed": true,
  "id": "my-server",
  "filename": "essentialsx.jar",
  "path": "./servers/my-server/plugins/essentialsx.jar",
  "size_bytes": 123456
}
```

### Mod Toggle Request / Response

**Request:** `PUT /api/instances/{id}/mods/{filename}/toggle`
```json
{
  "enabled": false
}
```

Disabling renames `{filename}` to `{filename}.jar.disabled`. Enabling does the reverse. No download needed.

### Generate Modpack Request / Response

**Request:** `POST /api/instances/{id}/mods/modpack`
```json
{
  "name": "My Modpack",
  "version": "1.0.0",
  "include": ["essentialsx.jar", "luckperms.jar"]
}
```

`include` is optional — defaults to all enabled mods.

**Response:**
```json
{
  "generated": true,
  "id": "my-server",
  "name": "My Modpack",
  "version": "1.0.0",
  "modpack_file": "./data/modpacks/my-server/my-modpack-1.0.0.mrpack",
  "size_bytes": 654321,
  "include_count": 2
}
```

### Instance Config (POST / PUT body)

```json
{
  "id": "my-server",
  "name": "My Survival Server",
  "provider": "fabric",
  "version": "1.21.4",
  "jar_path": "/srv/minecraft/servers/my-server/server.jar",
  "java_path": "",
  "min_memory": "1G",
  "max_memory": "4G",
  "jvm_args": ["-XX:+UseG1GC"],
  "loader_version": "0.16.10"
}
```

- `server_dir` is **always** derived server-side from `{settings.servers_dir}/{id}` — do not send it
- `java_path` defaults to `settings.java_path` when empty
- `loader_version` is optional. For `fabric` / `forge` / `neoforge` providers, if omitted the backend auto-fetches the **latest** loader version for the given MC version on creation.
- `mods` is a read-only cache populated automatically after install/delete/toggle operations.

### Instance Detail Response (GET)

```json
{
  "config": {
    "id": "my-server",
    "name": "My Survival Server",
    "provider": "fabric",
    "version": "1.21.4",
    "jar_path": "./servers/my-server/server.jar",
    "java_path": "/usr/bin/java",
    "min_memory": "1G",
    "max_memory": "4G",
    "jvm_args": ["-XX:+UseG1GC"],
    "server_dir": "./servers/my-server",
    "loader_version": "0.16.10",
    "mods": [
      {
        "filename": "sodium-fabric.jar",
        "name": "sodium-fabric",
        "enabled": true,
        "size_bytes": 123456,
        "size_human": "120.6 KB",
        "last_modified": "2025-01-15T12:00:00Z",
        "download_url": "https://cdn.modrinth.com/..."
      }
    ]
  },
  "status": {
    "id": "my-server",
    "name": "My Survival Server",
    "running": false,
    "online_players": [],
    "player_count": 0,
    "log_lines": 0,
    "properties_count": 0
  }
}
```

The `loader_version` and `mods` fields are persisted in the server's `.instance.json` file and survive restarts.

## Settings

```json
{
  "fail2ban_max_attempts": 5,
  "servers_dir": "./servers",
  "java_path": "/usr/bin/java"
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `fail2ban_max_attempts` | `5` | Max failed auth attempts before IP blacklist |
| `servers_dir` | `"./servers"` | Default directory for new server instances |
| `java_path` | `"/usr/bin/java"` | Default Java executable path |
| `fail2ban_enabled` | `true` | Enable/disable fail2ban IP blacklist |
| `ip_whitelist_enabled` | `false` | Enable/disable IP whitelist enforcement |
| `auto_stop_timeout_minutes` | `30` | Auto-stop idle servers after N minutes (0 = disabled) |

## Data Files

```
data/
├── app.db              SQLite database
├── settings.json       Backend settings
├── blacklist.json      Blacklisted IPs
└── instances.json      Server instance configs
```

## CLI Commands

```
backend serve              Start the HTTP API server
backend serve --daemon     Start in background (detached from terminal)
backend create-sudo        Create a sudo user (--email required)
backend list-users         List all registered users
backend ban-status         View blacklist and fail2ban config
backend unban <ip>         Remove an IP from the blacklist
backend reset-db           Drop all tables and reset everything (with confirmation)
backend install-service    Generate a systemd user service (easymc-server)
```

### systemd (Recommended for Production)

```bash
# Install the service (writes to ~/.config/systemd/user/easymc-server.service)
backend install-service

# Enable and start
systemctl --user daemon-reload
systemctl --user enable --now easymc-server

# View logs
journalctl --user -u easymc-server -f
```

The service auto-restarts on failure and survives terminal closes.
