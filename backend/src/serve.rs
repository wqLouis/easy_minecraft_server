use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::{
    extract::State,
    middleware as axum_middleware,
    routing::{get, post, put},
    Extension, Json, Router,
};
use serde_json::json;
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::auth::{me, register, AppState};
use crate::blacklist;
use crate::errors::AppError;
use crate::ip_ban::IpBanManager;
use crate::middleware::{check_ip_ban, require_auth, require_sudo};
use crate::models::User;
use crate::settings::{self as settings_mod, AppSettings};

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

    let state = Arc::new(AppState {
        db: pool,
        ip_ban: ip_ban.clone(),
        settings: settings.clone(),
        settings_path: settings_path.clone(),
        blacklist_path: blacklist_path.clone(),
    });

    log::info!(
        "Loaded {} blacklisted IP(s) from {}",
        blacklist_len,
        blacklist_path.display()
    );

    // ── No-auth routes ─────────────────────────────────────────────
    let public = Router::new().route("/api/health", get(health));

    // ── Auth required (valid API key) ──────────────────────────────
    let authed = Router::new()
        .route("/api/auth/me", get(me))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            check_ip_ban,
        ));

    // ── Sudo required ──────────────────────────────────────────────
    let sudo_routes = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/settings", get(get_settings))
        .route("/api/settings/schema", get(settings_schema))
        .route("/api/settings", put(update_settings))
        .route("/api/ipban/status", get(ipban_status))
        .route("/api/ipban/unban", post(ipban_unban))
        .route("/api/ipban/blacklist", post(ipban_add))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_sudo,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            check_ip_ban,
        ));

    let app = Router::new()
        .merge(public)
        .merge(authed)
        .merge(sudo_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
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

// ── Handlers ───────────────────────────────────────────────────────

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

/// Generate JSON Schema for settings dynamically.
async fn settings_schema() -> Json<serde_json::Value> {
    let schema = schemars::schema_for!(AppSettings);
    Json(serde_json::to_value(&schema).unwrap())
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Json(new): Json<AppSettings>,
) -> Result<Json<AppSettings>, AppError> {
    let updated =
        settings_mod::update_settings(&state.settings_path, state.settings.clone(), &state.ip_ban, new)?;
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

    // Persist the change to the blacklist file
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

    // Persist to file
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
