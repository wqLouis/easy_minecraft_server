use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::{
    extract::{Path, Query, State},
    http::HeaderName,
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth::{delete_user, list_users, me, register, update_user, AppState};
use crate::blacklist;
use crate::errors::AppError;
use crate::ip_ban::IpBanManager;
use crate::middleware::{check_ip_ban, require_auth, require_sudo};
use crate::models::User;
use crate::settings::{self as settings_mod, AppSettings};
use mc_server_manager::registry::{InstanceConfig, ServerRegistry};

// ---------------------------------------------------------------------------
// Server startup
// ---------------------------------------------------------------------------

/// Start the HTTP API server.
pub async fn serve(
    pool: SqlitePool,
    settings_path: PathBuf,
    blacklist_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // ── Load persistent settings ───────────────────────────────────
    let app_settings = settings_mod::load_settings(&settings_path);
    let fail2ban_max = app_settings.fail2ban_max_attempts;

    // ── Load persistent blacklist ──────────────────────────────────
    let loaded_blacklist = blacklist::load_blacklist(&blacklist_path)?;
    let blacklist_len = loaded_blacklist.len();

    let ip_ban = Arc::new(RwLock::new(IpBanManager::new(fail2ban_max)));
    let settings = Arc::new(RwLock::new(app_settings));

    // Restore blacklist into the manager
    {
        let mut ban = ip_ban.write().unwrap();
        for ip in &loaded_blacklist {
            ban.add_blacklist(ip);
        }
    }

    // ── Server instance registry ───────────────────────────────────
    let instances_path = blacklist_path
        .parent()
        .map(|p| p.join("instances.json"))
        .unwrap_or_else(|| PathBuf::from("./data/instances.json"));
    let registry = ServerRegistry::new(instances_path);

    let state = Arc::new(AppState {
        db: pool,
        ip_ban: ip_ban.clone(),
        settings: settings.clone(),
        settings_path: settings_path.clone(),
        blacklist_path: blacklist_path.clone(),
        server_registry: registry,
    });

    log::info!(
        "Loaded {} blacklisted IP(s) from {}",
        blacklist_len,
        blacklist_path.display()
    );

    // ── Routes requiring only auth ─────────────────────────────────
    let authed = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/me", get(me));

    // ── Sudo-only routes ───────────────────────────────────────────
    let sudo_routes = Router::new()
        // Auth / admin
        .route("/api/auth/register", post(register))
        // User management
        .route("/api/users", get(list_users))
        .route("/api/users/{id}", delete(delete_user))
        .route("/api/users/{id}", put(update_user))
        .route("/api/settings", get(get_settings))
        .route("/api/settings/schema", get(settings_schema))
        .route("/api/settings", put(update_settings))
        .route("/api/ipban/status", get(ipban_status))
        .route("/api/ipban/unban", post(ipban_unban))
        .route("/api/ipban/blacklist", post(ipban_add))
        // Providers / versions
        .route("/api/providers", get(list_providers))
        .route("/api/providers/{provider}/versions", get(fetch_versions_handler))
        .route(
            "/api/providers/{provider}/versions/{version}",
            get(fetch_version_info),
        )
        // Instance management
        .route("/api/instances", get(list_instances))
        .route("/api/instances", post(create_instance))
        .route("/api/instances/{id}", get(get_instance))
        .route("/api/instances/{id}", delete(remove_instance))
        .route("/api/instances/{id}/config", put(update_instance_config))
        .route("/api/instances/{id}/start", post(start_instance))
        .route("/api/instances/{id}/stop", post(stop_instance))
        .route("/api/instances/{id}/command", post(instance_command))
        .route("/api/instances/{id}/logs", get(instance_logs))
        .route("/api/instances/{id}/players", get(instance_players))
        .layer(axum_middleware::from_fn(require_sudo));

    // ── Merge and apply global auth ────────────────────────────────
    let app = Router::new()
        .merge(authed)
        .merge(sudo_routes)
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            check_ip_ban,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers([
                    HeaderName::from_static("content-type"),
                    HeaderName::from_static("authorization"),
                ])
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind address");

    log::info!("Backend API listening on http://0.0.0.0:3000");

    axum::serve(listener, app)
        .await
        .expect("Server failed");

    Ok(())
}

// ── Health ─────────────────────────────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

// ── Settings ───────────────────────────────────────────────────────

async fn get_settings(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
) -> Result<Json<AppSettings>, AppError> {
    let s = state
        .settings
        .read()
        .map_err(|e| AppError::Internal(format!("Settings lock error: {}", e)))?;
    Ok(Json(s.clone()))
}

async fn settings_schema() -> Json<serde_json::Value> {
    let schema = schemars::schema_for!(AppSettings);
    Json(serde_json::to_value(&schema).unwrap())
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Json(new): Json<AppSettings>,
) -> Result<Json<AppSettings>, AppError> {
    let updated = settings_mod::update_settings(
        &state.settings_path,
        state.settings.clone(),
        &state.ip_ban,
        new,
    )?;
    Ok(Json(updated))
}

// ── IP ban management ──────────────────────────────────────────────

async fn ipban_status(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
) -> Result<Json<serde_json::Value>, AppError> {
    let status = state.ip_ban.read().unwrap().status();
    Ok(Json(json!(status)))
}

async fn ipban_unban(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Json(body): Json<UnbanRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let removed = state.ip_ban.write().unwrap().unban(&body.ip);

    if removed {
        let ips: Vec<String> = state.ip_ban.read().unwrap().blacklist().to_vec();
        blacklist::save_blacklist(&state.blacklist_path, &ips)?;
        log::info!("IP {} removed from blacklist", body.ip);
    }

    Ok(Json(json!({"unbanned": removed, "ip": body.ip})))
}

async fn ipban_add(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Json(body): Json<BlacklistRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    {
        let mut ban = state.ip_ban.write().unwrap();
        ban.add_blacklist(&body.ip);
    }

    let ips: Vec<String> = state.ip_ban.read().unwrap().blacklist().to_vec();
    blacklist::save_blacklist(&state.blacklist_path, &ips)?;
    log::info!("IP {} added to blacklist", body.ip);

    Ok(Json(json!({"blacklisted": true, "ip": body.ip})))
}

#[derive(serde::Deserialize)]
struct UnbanRequest {
    ip: String,
}

#[derive(serde::Deserialize)]
struct BlacklistRequest {
    ip: String,
}

// ═══════════════════════════════════════════════════════════════════
// Server Version Providers
// ═══════════════════════════════════════════════════════════════════

async fn list_providers() -> Json<serde_json::Value> {
    let providers = mc_server_manager::list_providers();
    Json(json!(providers))
}

async fn fetch_versions_handler(
    Path(provider): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let versions = mc_server_manager::fetch_versions(&provider)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "provider": provider, "versions": versions })))
}

async fn fetch_version_info(
    Path((provider, version)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let info = mc_server_manager::fetch_latest(&provider, &version)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!(info)))
}

// ═══════════════════════════════════════════════════════════════════
// Server Instance Management
// ═══════════════════════════════════════════════════════════════════

/// List all server instances.
async fn list_instances(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
) -> Json<serde_json::Value> {
    let instances = state.server_registry.list();
    Json(json!(instances))
}

/// Create a new server instance.
async fn create_instance(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Json(mut config): Json<InstanceConfig>,
) -> Result<Json<serde_json::Value>, AppError> {
    let settings = state
        .settings
        .read()
        .map_err(|e| AppError::Internal(format!("Settings lock: {e}")))?;

    // Always derive server_dir from settings — user input is ignored
    config.server_dir = format!("{}/{}", settings.servers_dir.trim_end_matches('/'), config.id);

    // Default java_path from settings if not provided
    if config.java_path.is_empty() {
        config.java_path = settings.java_path.clone();
    }
    drop(settings);

    state
        .server_registry
        .create(config.clone())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "created": true, "instance": config })))
}

/// Get details for a single instance.
async fn get_instance(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (config, handle) = state
        .server_registry
        .get_info(&id)
        .ok_or_else(|| AppError::Internal(format!("Instance '{id}' not found")))?;

    let status = handle.status().await;
    Ok(Json(json!({
        "config": config,
        "status": status,
    })))
}

/// Remove a server instance.
async fn remove_instance(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .server_registry
        .remove(&id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "removed": true, "id": id })))
}

/// Update instance configuration.
async fn update_instance_config(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
    Json(mut config): Json<InstanceConfig>,
) -> Result<Json<serde_json::Value>, AppError> {
    let settings = state
        .settings
        .read()
        .map_err(|e| AppError::Internal(format!("Settings lock: {e}")))?;

    // Always derive server_dir from settings
    config.server_dir = format!("{}/{}", settings.servers_dir.trim_end_matches('/'), &id);
    if config.java_path.is_empty() {
        config.java_path = settings.java_path.clone();
    }
    drop(settings);

    state
        .server_registry
        .update_config(&id, config)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "updated": true, "id": id })))
}

/// Start a server instance.
async fn start_instance(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .server_registry
        .start(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "started": true, "id": id })))
}

/// Stop a server instance.
async fn stop_instance(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .server_registry
        .stop(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "stopped": true, "id": id })))
}

/// Send a console command to a running instance.
async fn instance_command(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<CommandRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .server_registry
        .send_command(&id, &body.command)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "sent": true, "id": id, "command": body.command })))
}

#[derive(Deserialize)]
struct CommandRequest {
    command: String,
}

/// Get logs for an instance.
async fn instance_logs(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let logs = server.handle().logs_tail(query.tail.unwrap_or(100)).await;
    Ok(Json(json!({ "id": id, "logs": logs, "count": logs.len() })))
}

#[derive(Deserialize)]
struct LogsQuery {
    tail: Option<usize>,
}

/// Get online players for an instance.
async fn instance_players(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let players = server.handle().online_players().await;
    Ok(Json(json!({
        "id": id,
        "players": players,
        "count": players.len(),
    })))
}
