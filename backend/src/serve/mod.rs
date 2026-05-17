//! HTTP API server — routing and setup.
pub mod instances;
pub mod mods;
pub mod ram;
pub mod worlds;

use crate::auth::{AppState, delete_user, list_users, me, register, update_user};
use crate::errors::AppError;
use crate::ip_ban;
use crate::middleware::{check_ip_ban, require_auth, require_sudo};
use crate::models::User;
use crate::settings::{self as settings_mod, AppSettings};
use axum::{
    Extension, Json, Router,
    extract::State,
    http::HeaderName,
    middleware as mw,
    routing::{delete, get, post, put},
};
use mc_server_manager::registry::ServerRegistry;
use serde_json::json;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub async fn serve(
    mut pool: SqlitePool,
    settings_path: PathBuf,
    blacklist_path: PathBuf,
    tmpfs: bool,
    _size: Option<String>,
    tmpfs_path: Option<String>,
    database_url: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_settings = settings_mod::load_settings(&settings_path);
    let ip_ban_mgr = Arc::new(std::sync::RwLock::new(crate::ip_ban::IpBanManager::new(
        app_settings.fail2ban_max_attempts,
    )));
    let settings = Arc::new(std::sync::RwLock::new(app_settings));
    let loaded = ip_ban::load_blacklist(&blacklist_path)?;
    {
        let mut b = ip_ban_mgr.write().unwrap();
        for ip in &loaded {
            b.add_blacklist(ip);
        }
    }

    let ramdisk: Option<ram::RamDisk> = if tmpfs {
        let default_data = PathBuf::from("./data");
        let data_dir = settings_path.parent().unwrap_or(&default_data);
        let servers_dir = {
            let s = settings.read().map_err(|e| format!("Settings lock: {e}"))?;
            PathBuf::from(s.servers_dir.clone())
        };
        let rd = ram::RamDisk::setup(tmpfs_path.as_deref(), data_dir, &servers_dir)?;
        pool.close().await;
        let new_url = database_url.replacen(
            data_dir.to_string_lossy().as_ref(),
            rd.data_dir.to_string_lossy().as_ref(),
            1,
        );
        pool = crate::db::init_pool(&new_url)
            .await
            .map_err(|e| format!("DB re-init on tmpfs: {e}"))?;
        {
            let mut s = settings
                .write()
                .map_err(|e| format!("Settings lock: {e}"))?;
            s.servers_dir = rd.servers_dir.to_string_lossy().to_string();
        }
        log::info!(
            "Tmpfs ready: data {}, servers {}",
            rd.data_dir.display(),
            rd.servers_dir.display()
        );
        Some(rd)
    } else {
        None
    };

    let active_sp = ramdisk
        .as_ref()
        .map(|r| r.data_dir.join("settings.json"))
        .unwrap_or(settings_path);
    let active_bp = ramdisk
        .as_ref()
        .map(|r| r.data_dir.join("blacklist.json"))
        .unwrap_or(blacklist_path);
    let ip = active_bp
        .parent()
        .map(|p| p.join("instances.json"))
        .unwrap_or_else(|| PathBuf::from("./data/instances.json"));
    let registry = ServerRegistry::new(ip);
    let sr = registry.clone();

    if let Some(ref rd) = ramdisk {
        for s in registry.list() {
            if let Some((cfg, _)) = registry.get_info(&s.id) {
                if cfg
                    .server_dir
                    .starts_with(rd.original_servers_dir.to_string_lossy().as_ref())
                {
                    let mut u = cfg.clone();
                    u.server_dir = cfg.server_dir.replacen(
                        rd.original_servers_dir.to_string_lossy().as_ref(),
                        rd.servers_dir.to_string_lossy().as_ref(),
                        1,
                    );
                    u.jar_path = format!("{}/server.jar", u.server_dir);
                    let _ = registry.update_config(&s.id, u);
                }
            }
        }
    }

    let state = Arc::new(AppState {
        db: pool,
        ip_ban: ip_ban_mgr,
        settings: settings.clone(),
        settings_path: active_sp,
        blacklist_path: active_bp,
        server_registry: registry,
        rate_limiter: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    });
    log::info!("Loaded {} blacklisted IP(s)", loaded.len());

    #[rustfmt::skip]
    let sudo = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/users", get(list_users)).route("/api/users/{id}", delete(delete_user)).route("/api/users/{id}", put(update_user))
        .route("/api/settings", get(get_settings)).route("/api/settings/schema", get(settings_schema)).route("/api/settings", put(update_settings))
        .route("/api/ipban/status", get(ipban_status)).route("/api/ipban/unban", post(ipban_unban)).route("/api/ipban/blacklist", post(ipban_add))
        .route("/api/providers", get(instances::list_providers)).route("/api/providers/{provider}/versions", get(instances::fetch_versions)).route("/api/providers/{provider}/versions/{version}", get(instances::fetch_version_info))
        .route("/api/instances", get(instances::list_instances)).route("/api/instances", post(instances::create_instance))
        .route("/api/instances/{id}", get(instances::get_instance)).route("/api/instances/{id}", delete(instances::remove_instance)).route("/api/instances/{id}/config", put(instances::update_instance_config))
        .route("/api/instances/{id}/start", post(instances::start_instance)).route("/api/instances/{id}/stop", post(instances::stop_instance)).route("/api/instances/{id}/command", post(instances::instance_command))
        .route("/api/instances/{id}/logs", get(instances::instance_logs)).route("/api/instances/{id}/players", get(instances::instance_players)).route("/api/instances/{id}/logs/stream", get(instances::log_stream))
        .route("/api/instances/{id}/properties", get(instances::get_properties)).route("/api/instances/{id}/properties", put(instances::update_properties))
        .route("/api/instances/schema", get(instances::instance_config_schema)).route("/api/instances/archived", get(instances::list_archived)).route("/api/instances/archived/{id}/restore", post(instances::restore_archived))
        .route("/api/instances/{id}/command/history", get(instances::command_history))
        .route("/api/instances/{id}/worlds", get(worlds::list_worlds)).route("/api/instances/{id}/worlds/backup", post(worlds::backup_worlds))
        .route("/api/instances/{id}/backups", get(worlds::list_backups)).route("/api/instances/{id}/worlds/upload", post(worlds::upload_world)).route("/api/instances/{id}/worlds/{world_name}", delete(worlds::delete_world))
        .route("/api/instances/{id}/worlds/{world_name}/download", get(worlds::download_world)).route("/api/instances/{id}/worlds/reset", post(worlds::reset_world))
        .route("/api/modrinth/search", get(mods::modrinth_search)).route("/api/modrinth/project/{slug}", get(mods::modrinth_project)).route("/api/modrinth/project/{slug}/versions", get(mods::modrinth_project_versions)).route("/api/modrinth/project/{slug}/download-url", get(mods::modrinth_download_url))
        .route("/api/instances/{id}/mods", get(mods::list_mods)).route("/api/instances/{id}/mods/install", post(mods::install_mod))
        .route("/api/instances/{id}/mods/{filename}", delete(mods::delete_mod)).route("/api/instances/{id}/mods/{filename}/toggle", put(mods::toggle_mod))
        .route("/api/instances/{id}/mods/modpack", post(mods::generate_modpack)).route("/api/instances/{id}/mods/modpack/download", get(mods::download_modpack))
        .layer(mw::from_fn(require_sudo));

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/me", get(me))
        .route("/api/ipwhitelist", get(whitelist_status_handler))
        .merge(sudo)
        .layer(mw::from_fn_with_state(state.clone(), require_auth))
        .layer(mw::from_fn_with_state(state.clone(), check_ip_ban))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers([
                    HeaderName::from_static("content-type"),
                    HeaderName::from_static("authorization"),
                ]),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind address");
    log::info!("Backend API listening on http://0.0.0.0:3000");

    let sig = {
        let rd = ramdisk.clone();
        async move {
            let ctrl_c = tokio::signal::ctrl_c();
            #[cfg(unix)]
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("SIGTERM handler");
            #[cfg(unix)]
            tokio::select! { _ = ctrl_c => {} _ = term.recv() => {} }
            #[cfg(not(unix))]
            ctrl_c.await.expect("Ctrl+C");
            log::info!("Shutting down...");
            if let Some(rd) = rd {
                log::info!("Syncing tmpfs...");
                let _ = rd.sync_back();
                rd.cleanup();
            }
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(sig)
        .await
        .expect("Server failed");
    log::info!("Killed {} server instance(s)", sr.kill_all().await);
    Ok(())
}

// ── Admin handlers ────────────────────────────────────────────────
async fn health() -> &'static str {
    "ok"
}
async fn get_settings(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
) -> Result<Json<AppSettings>, AppError> {
    Ok(Json(
        s.settings
            .read()
            .map_err(|e| AppError::Internal(format!("Settings lock: {e}")))
            .unwrap()
            .clone(),
    ))
}
async fn settings_schema() -> Json<serde_json::Value> {
    Json(serde_json::to_value(&schemars::schema_for!(AppSettings)).unwrap())
}
async fn update_settings(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Json(n): Json<AppSettings>,
) -> Result<Json<AppSettings>, AppError> {
    if n.servers_dir.contains("..") || n.servers_dir.contains('~') || n.servers_dir.starts_with('/')
    {
        return Err(AppError::Internal(
            "servers_dir must be a relative path without '..' or '~'".into(),
        ));
    }
    if !n.java_path.contains("java") || n.java_path.contains("..") {
        return Err(AppError::Internal(
            "java_path must point to a Java executable".into(),
        ));
    }
    settings_mod::update_settings(&s.settings_path, s.settings.clone(), &s.ip_ban, n).map(Json)
}
async fn ipban_status(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!(s.ip_ban.read().unwrap().status())))
}
async fn ipban_unban(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Json(b): Json<UnbanReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    if s.ip_ban.write().unwrap().unban(&b.ip) {
        ip_ban::save_blacklist(
            &s.blacklist_path,
            &s.ip_ban.read().unwrap().blacklist().to_vec(),
        )?;
        log::info!("IP {} removed from blacklist", b.ip);
    }
    Ok(Json(json!({"unbanned": true, "ip": b.ip})))
}
async fn ipban_add(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Json(b): Json<BlacklistReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    s.ip_ban.write().unwrap().add_blacklist(&b.ip);
    ip_ban::save_blacklist(
        &s.blacklist_path,
        &s.ip_ban.read().unwrap().blacklist().to_vec(),
    )?;
    log::info!("IP {} added to blacklist", b.ip);
    Ok(Json(json!({"blacklisted": true, "ip": b.ip})))
}
#[derive(serde::Deserialize)]
struct UnbanReq {
    ip: String,
}
#[derive(serde::Deserialize)]
struct BlacklistReq {
    ip: String,
}

async fn whitelist_status_handler(
    State(s): State<Arc<AppState>>,
    Extension(u): Extension<User>,
) -> Result<Json<serde_json::Value>, AppError> {
    use chrono::{TimeZone, Utc as CtUtc};
    let enabled = s
        .settings
        .read()
        .map(|s| s.ip_whitelist_enabled)
        .unwrap_or(false);
    if !enabled {
        return Ok(Json(json!({"enabled": false})));
    }
    let cutoff = (CtUtc::now() - chrono::Duration::hours(12))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let row: Option<(String,)> =
        sqlx::query_as("SELECT updated_at FROM ip_whitelist WHERE user_id = ? AND updated_at >= ?")
            .bind(&u.id)
            .bind(&cutoff)
            .fetch_optional(&s.db)
            .await?;
    let (remaining, updated_at) = match row {
        Some((ts,)) => chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S")
            .ok()
            .and_then(|dt| {
                let expiry = CtUtc.from_utc_datetime(&dt) + chrono::Duration::hours(12);
                Some(((expiry - CtUtc::now()).num_seconds().max(0), ts.clone()))
            })
            .unwrap_or((0, ts)),
        None => (0, String::new()),
    };
    Ok(Json(
        json!({"enabled": true, "active": remaining > 0, "remaining_secs": remaining, "remaining_hours": remaining as f64 / 3600.0, "updated_at": updated_at}),
    ))
}
