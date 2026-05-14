# Easy MC Server — Backend

Monorepo for a Rust backend API that hosts and controls Minecraft servers.

## Project structure

```
Cargo.toml                 # Workspace root
backend/
├── Cargo.toml             # HTTP API server (axum + sqlite)
├── src/                   # Auth, middleware, serve, settings, ...
├── docs/api.md            # Full API endpoint reference
├── data/                  # Runtime data (app.db, settings.json, blacklist.json)
└── mc-server-manager/     # Library crate: spawn & control MC server processes
    ├── Cargo.toml
    └── src/lib.rs         # ServerConfig, ServerInstance API
```

## Quick start

```bash
# Build everything from the workspace root
cargo build

# Create the first admin user
./target/debug/backend create-sudo --email admin@example.com

# Start the API server
RUST_LOG=info ./target/debug/backend serve
```

See [`backend/docs/api.md`](backend/docs/api.md) for the full API reference.
See [`backend/docs/external_apis.md`](backend/docs/external_apis.md) for server-software download APIs.
See [`backend/docs/server_software_guide.md`](backend/docs/server_software_guide.md) for how to obtain and install each server platform.
See [`backend/docs/mc-server-installer.md`](backend/docs/mc-server-installer.md) for the installer crate API.

## Workspace members

| Crate | Path | Description |
|-------|------|-------------|
| `backend` | `backend/` | HTTP API server — auth, fail2ban, settings |
| `mc-server-manager` | `backend/src/mc-server-manager/` | Library for launching/managing MC server JAR processes |
| `mc-server-installer` | `backend/src/mc-server-installer/` | Library for downloading MC server software from multiple sources |
