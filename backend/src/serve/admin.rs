//! Admin-only handlers — settings, IP ban, whitelist status.
use crate::auth::AppState;
use crate::errors::AppError;
use crate::ip_ban;
use crate::models::User;
use crate::settings::{self as settings_mod, AppSettings};
use axum::{Extension, Json, extract::State};
use serde_json::json;
use std::sync::Arc;

// ── Settings ──────────────────────────────────────────────────────

pub async fn get_settings(
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

pub async fn settings_schema() -> Json<serde_json::Value> {
    Json(serde_json::to_value(&schemars::schema_for!(AppSettings)).unwrap())
}

pub async fn update_settings(
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

// ── IP Ban ────────────────────────────────────────────────────────

pub async fn ipban_status(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!(s.ip_ban.read().unwrap().status())))
}

#[derive(serde::Deserialize)]
pub struct UnbanReq {
    ip: String,
}

pub async fn ipban_unban(
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

#[derive(serde::Deserialize)]
pub struct BlacklistReq {
    ip: String,
}

pub async fn ipban_add(
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

// ── IP Whitelist ──────────────────────────────────────────────────

pub async fn whitelist_status(
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

// ── Health ────────────────────────────────────────────────────────

pub async fn health() -> &'static str {
    "ok"
}
