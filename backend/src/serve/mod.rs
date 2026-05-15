//! API server — HTTP handlers, routing, and middleware setup.
//!
//! Submodules:
//! - [`instances`] — instance CRUD, worlds, providers, properties, archive, logs
//! - [`mods`] — Modrinth API, mod/plugin management, modpack generation

pub mod instances;
pub mod mods;

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::State,
    http::HeaderName,
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use serde_json::json;
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth::{delete_user, list_users, me, register, update_user, AppState};
use crate::errors::AppError;
use crate::ip_ban;
use crate::middleware::{check_ip_ban, require_auth, require_sudo};
use crate::models::User;
use crate::settings::{self as settings_mod, AppSettings};
use mc_server_manager::registry::ServerRegistry;

/// Start the HTTP API server.
pub async fn serve(
    pool: SqlitePool,
    settings_path: PathBuf,
    blacklist_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_settings = settings_mod::load_settings(&settings_path);
    let fail2ban_max = app_settings.fail2ban_max_attempts;

    let loaded_blacklist = ip_ban::load_blacklist(&blacklist_path)?;
    let blacklist_len = loaded_blacklist.len();

    let ip_ban_mgr = Arc::new(std::sync::RwLock::new(
        crate::ip_ban::IpBanManager::new(fail2ban_max),
    ));
    let settings = Arc::new(std::sync::RwLock::new(app_settings));

    {
        let mut ban = ip_ban_mgr.write().unwrap();
        for ip in &loaded_blacklist {
            ban.add_blacklist(ip);
        }
    }

    let instances_path = blacklist_path
        .parent()
        .map(|p| p.join("instances.json"))
        .unwrap_or_else(|| PathBuf::from("./data/instances.json"));

    let state = Arc::new(AppState {
        db: pool,
        ip_ban: ip_ban_mgr.clone(),
        settings: settings.clone(),
        settings_path: settings_path.clone(),
        blacklist_path: blacklist_path.clone(),
        server_registry: ServerRegistry::new(instances_path),
    });

    log::info!("Loaded {blacklist_len} blacklisted IP(s) from {}", blacklist_path.display());

    let authed = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/me", get(me))
        .route("/api/ipwhitelist", get(whitelist_status_handler))
        .route("/api/instances/{id}/mods/modpack/download", get(mods::download_modpack));

    let sudo_routes = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/users", get(list_users))
        .route("/api/users/{id}", delete(delete_user))
        .route("/api/users/{id}", put(update_user))
        .route("/api/settings", get(get_settings))
        .route("/api/settings/schema", get(settings_schema))
        .route("/api/settings", put(update_settings))
        .route("/api/ipban/status", get(ipban_status))
        .route("/api/ipban/unban", post(ipban_unban))
        .route("/api/ipban/blacklist", post(ipban_add))
        .route("/api/providers", get(instances::list_providers))
        .route("/api/providers/{provider}/versions", get(instances::fetch_versions_handler))
        .route("/api/providers/{provider}/versions/{version}", get(instances::fetch_version_info))
        .route("/api/instances", get(instances::list_instances))
        .route("/api/instances", post(instances::create_instance))
        .route("/api/instances/{id}", get(instances::get_instance))
        .route("/api/instances/{id}", delete(instances::remove_instance))
        .route("/api/instances/{id}/config", put(instances::update_instance_config))
        .route("/api/instances/{id}/start", post(instances::start_instance))
        .route("/api/instances/{id}/stop", post(instances::stop_instance))
        .route("/api/instances/{id}/command", post(instances::instance_command))
        .route("/api/instances/{id}/logs", get(instances::instance_logs))
        .route("/api/instances/{id}/players", get(instances::instance_players))
        .route("/api/instances/{id}/worlds", get(instances::list_worlds))
        .route("/api/instances/{id}/worlds/backup", post(instances::backup_worlds))
        .route("/api/instances/{id}/backups", get(instances::list_backups))
        .route("/api/instances/{id}/worlds/upload", post(instances::upload_world))
        .route("/api/instances/{id}/worlds/reset", post(instances::reset_world))
        .route("/api/instances/{id}/worlds/{world_name}", delete(instances::delete_world))
        .route("/api/instances/{id}/worlds/{world_name}/download", get(instances::download_world))
        .route("/api/instances/{id}/properties", get(instances::get_properties))
        .route("/api/instances/{id}/properties", put(instances::update_properties))
        .route("/api/instances/{id}/logs/stream", get(instances::log_stream))
        .route("/api/instances/schema", get(instances::instance_config_schema))
        .route("/api/instances/archived", get(instances::list_archived_instances))
        .route("/api/instances/archived/{id}/restore", post(instances::restore_archived_instance))
        .route("/api/instances/{id}/command/history", get(instances::command_history))
        .route("/api/modrinth/search", get(mods::modrinth_search))
        .route("/api/modrinth/project/{slug}/versions", get(mods::modrinth_project_versions))
        .route("/api/modrinth/project/{slug}/download-url", get(mods::modrinth_download_url))
        .route("/api/instances/{id}/mods", get(mods::list_mods))
        .route("/api/instances/{id}/mods/install", post(mods::install_mod))
        .route("/api/instances/{id}/mods/{filename}", delete(mods::delete_mod))
        .route("/api/instances/{id}/mods/{filename}/toggle", put(mods::toggle_mod))
        .route("/api/instances/{id}/mods/modpack", post(mods::generate_modpack))
        .layer(axum_middleware::from_fn(require_sudo));

    let app = Router::new()
        .merge(authed)
        .merge(sudo_routes)
        .layer(axum_middleware::from_fn_with_state(state.clone(), require_auth))
        .layer(axum_middleware::from_fn_with_state(state.clone(), check_ip_ban))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
        ]))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind address");
    log::info!("Backend API listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.expect("Server failed");
    Ok(())
}

// ── Admin handlers ────────────────────────────────────────────────

async fn health() -> &'static str { "ok" }

async fn get_settings(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
) -> Result<Json<AppSettings>, AppError> {
    let s = state.settings.read().map_err(|e| AppError::Internal(format!("Settings lock: {e}")))?;
    Ok(Json(s.clone()))
}

async fn settings_schema() -> Json<serde_json::Value> {
    Json(serde_json::to_value(&schemars::schema_for!(AppSettings)).unwrap())
}

async fn update_settings(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Json(new): Json<AppSettings>,
) -> Result<Json<AppSettings>, AppError> {
    settings_mod::update_settings(&state.settings_path, state.settings.clone(), &state.ip_ban, new).map(Json)
}

async fn ipban_status(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!(state.ip_ban.read().unwrap().status())))
}

async fn ipban_unban(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Json(body): Json<UnbanRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if state.ip_ban.write().unwrap().unban(&body.ip) {
        ip_ban::save_blacklist(&state.blacklist_path, &state.ip_ban.read().unwrap().blacklist().to_vec())?;
        log::info!("IP {} removed from blacklist", body.ip);
    }
    Ok(Json(json!({"unbanned": true, "ip": body.ip})))
}

async fn ipban_add(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Json(body): Json<BlacklistRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.ip_ban.write().unwrap().add_blacklist(&body.ip);
    ip_ban::save_blacklist(&state.blacklist_path, &state.ip_ban.read().unwrap().blacklist().to_vec())?;
    log::info!("IP {} added to blacklist", body.ip);
    Ok(Json(json!({"blacklisted": true, "ip": body.ip})))
}

#[derive(serde::Deserialize)]
struct UnbanRequest { ip: String }

#[derive(serde::Deserialize)]
struct BlacklistRequest { ip: String }

async fn whitelist_status_handler(
    State(state): State<Arc<AppState>>, Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>, AppError> {
    use chrono::{TimeZone, Utc as ChronoUtc};
    let enabled = state.settings.read().map(|s| s.ip_whitelist_enabled).unwrap_or(false);
    if !enabled { return Ok(Json(json!({"enabled": false}))); }

    let cutoff = (ChronoUtc::now() - chrono::Duration::hours(12))
        .format("%Y-%m-%d %H:%M:%S").to_string();

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT updated_at FROM ip_whitelist WHERE user_id = ? AND updated_at >= ?",
    ).bind(&user.id).bind(&cutoff).fetch_optional(&state.db).await?;

    let (remaining, updated_at) = match row {
        Some((ts,)) => chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S").ok()
            .and_then(|dt| {
                let expiry = ChronoUtc.from_utc_datetime(&dt) + chrono::Duration::hours(12);
                Some(((expiry - ChronoUtc::now()).num_seconds().max(0), ts.clone()))
            })
            .unwrap_or((0, ts)),
        None => (0, String::new()),
    };

    Ok(Json(json!({"enabled": true, "active": remaining > 0, "remaining_secs": remaining,
        "remaining_hours": remaining as f64 / 3600.0, "updated_at": updated_at})))
}
