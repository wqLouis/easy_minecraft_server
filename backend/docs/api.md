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

### Instance Config (POST / PUT body)

```json
{
  "id": "my-server",
  "name": "My Survival Server",
  "jar_path": "/srv/minecraft/servers/my-server/server.jar",
  "java_path": "",
  "min_memory": "1G",
  "max_memory": "4G",
  "jvm_args": ["-XX:+UseG1GC"]
}
```

- `server_dir` is **always** derived server-side from `{settings.servers_dir}/{id}` — do not send it
- If `java_path` is empty, `settings.java_path` is used

### Instance Detail Response (GET)

```json
{
  "config": { "...": "same as above" },
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
