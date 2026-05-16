//! Authentication — credential helpers, handlers, IP whitelist.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use argon2::{password_hash::{rand_core::OsRng, SaltString}, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use chrono::Utc;
use rand::Rng;
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;
use crate::errors::AppError;
use crate::ip_ban::IpBanManager;
use crate::models::*;
use crate::settings::AppSettings;
use mc_server_manager::registry::ServerRegistry;

const W_TTL: i64 = 12 * 60 * 60;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub ip_ban: Arc<RwLock<IpBanManager>>,
    pub settings: Arc<RwLock<AppSettings>>,
    pub settings_path: PathBuf,
    pub blacklist_path: PathBuf,
    pub server_registry: ServerRegistry,
    pub rate_limiter: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}
impl AppState {
    pub fn get_server(&self, id: &str) -> Result<mc_server_manager::ManagedServer, AppError> {
        self.server_registry.get_server(id).map_err(|e| AppError::Internal(format!("Instance '{id}': {e}")))
    }
}

pub fn generate_token() -> String { hex::encode(rand::thread_rng().r#gen::<[u8; 32]>()) }
fn hash_cred(token: &str, username: &str) -> Result<String, AppError> {
    Ok(Argon2::default().hash_password(format!("{token}{username}").as_bytes(), &SaltString::generate(&mut OsRng))?.to_string())
}
fn verify_cred(token: &str, username: &str, hash: &str) -> Result<bool, AppError> {
    Ok(Argon2::default().verify_password(format!("{token}{username}").as_bytes(), &PasswordHash::new(hash)?).is_ok())
}

// ── Handlers ──────────────────────────────────────────────────────
pub async fn register(State(s): State<Arc<AppState>>, Extension(_r): Extension<User>, Json(body): Json<RegisterRequest>) -> Result<impl IntoResponse, AppError> {
    let username = body.username.trim().to_lowercase();
    if username.is_empty() || username.len() < 2 { return Err(AppError::Internal("Username must be ≥2 chars".into())); }
    if !username.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') { return Err(AppError::Internal("Alphanumeric, -, _ only".into())); }
    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = ?").bind(&username).fetch_one(&s.db).await?;
    if existing > 0 { return Err(AppError::UsernameAlreadyExists); }
    let token = generate_token();
    let hash = hash_cred(&token, &username)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query("INSERT INTO users (id, username, api_key_hash, is_sudoer, created_at, updated_at) VALUES (?, ?, ?, 0, ?, ?)").bind(&id).bind(&username).bind(&hash).bind(&now).bind(&now).execute(&s.db).await?;
    Ok((StatusCode::CREATED, Json(json!(CreatedUserResponse { user: UserResponse::from(User { id, username, api_key_hash: hash, is_sudoer: false, created_at: now.clone(), updated_at: now }), api_key: token }))))
}

pub async fn me(Extension(u): Extension<User>) -> impl IntoResponse { Json(MeResponse { user: UserResponse::from(u) }) }

pub async fn list_users(State(s): State<Arc<AppState>>, Extension(_u): Extension<User>) -> Result<Json<Vec<UserResponse>>, AppError> {
    Ok(Json(sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at ASC").fetch_all(&s.db).await?.into_iter().map(UserResponse::from).collect()))
}

pub async fn delete_user(State(s): State<Arc<AppState>>, Extension(r): Extension<User>, axum::extract::Path(id): axum::extract::Path<String>) -> Result<impl IntoResponse, AppError> {
    if r.id == id { return Err(AppError::Internal("Cannot delete yourself".into())); }
    if sqlx::query("DELETE FROM users WHERE id = ?").bind(&id).execute(&s.db).await?.rows_affected() == 0 { return Err(AppError::Internal(format!("User '{id}' not found"))); }
    Ok((StatusCode::OK, Json(json!({"deleted": true, "id": id}))))
}

#[derive(serde::Deserialize)] pub struct UpdateUserReq { pub username: Option<String> }
pub async fn update_user(State(s): State<Arc<AppState>>, Extension(_r): Extension<User>, axum::extract::Path(id): axum::extract::Path<String>, Json(body): Json<UpdateUserReq>) -> Result<Json<UserResponse>, AppError> {
    let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?").bind(&id).fetch_optional(&s.db).await?.ok_or_else(|| AppError::Internal(format!("User '{id}' not found")))?;
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let name = body.username.unwrap_or(existing.username);
    sqlx::query("UPDATE users SET username = ?, updated_at = ? WHERE id = ?").bind(&name).bind(&now).bind(&id).execute(&s.db).await?;
    Ok(Json(UserResponse::from(User { username: name, updated_at: now, ..existing })))
}

pub fn extract_credentials(headers: &axum::http::HeaderMap) -> Result<(String, String), AppError> {
    let auth = headers.get("Authorization").ok_or(AppError::MissingAuthHeader)?.to_str().map_err(|_| AppError::InvalidAuthHeader)?;
    let p = auth.strip_prefix("Bearer ").ok_or(AppError::InvalidAuthHeader)?.trim();
    let (username, token) = p.split_once(':').ok_or(AppError::InvalidAuthHeader)?;
    let (u, t) = (username.trim(), token.trim());
    if u.is_empty() || t.is_empty() { return Err(AppError::InvalidAuthHeader); }
    Ok((u.to_string(), t.to_string()))
}

pub async fn resolve_user(db: &SqlitePool, username: &str, token: &str) -> Result<User, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?").bind(username).fetch_optional(db).await?.ok_or(AppError::ApiKeyNotFound)?;
    if verify_cred(token, username, &user.api_key_hash)? { Ok(user) } else { Err(AppError::ApiKeyNotFound) }
}

pub fn client_ip(req: &axum::http::Request<axum::body::Body>, trust_proxy: bool) -> Option<String> {
    if trust_proxy {
        if let Some(val) = req.headers().get("x-forwarded-for") { if let Ok(s) = val.to_str() { if let Some(ip) = s.split(',').next() { let ip = ip.trim().to_string(); if !ip.is_empty() { return Some(ip); } } } }
    }
    req.extensions().get::<std::net::SocketAddr>().map(|a| a.ip().to_string())
}

pub async fn check_ip_whitelist(db: &SqlitePool, uid: &str, ip: &str, enabled: bool) -> Result<(), AppError> {
    if !enabled { return Ok(()); }
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let cutoff = (chrono::Utc::now() - chrono::Duration::seconds(W_TTL)).format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query("DELETE FROM ip_whitelist WHERE user_id = ? AND updated_at < ?").bind(uid).bind(&cutoff).execute(db).await?;
    let active = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ip_whitelist WHERE user_id = ? AND ip = ? AND updated_at >= ?").bind(uid).bind(ip).bind(&cutoff).fetch_one(db).await?;
    if active > 0 { sqlx::query("UPDATE ip_whitelist SET updated_at = ? WHERE user_id = ? AND ip = ?").bind(&now).bind(uid).bind(ip).execute(db).await?; return Ok(()); }
    let any = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ip_whitelist WHERE user_id = ? AND updated_at >= ?").bind(uid).bind(&cutoff).fetch_one(db).await?;
    if any == 0 { sqlx::query("INSERT INTO ip_whitelist (id, user_id, ip, updated_at) VALUES (?, ?, ?, ?)").bind(Uuid::new_v4().to_string()).bind(uid).bind(ip).bind(&now).execute(db).await?; return Ok(()); }
    Err(AppError::IpNotWhitelisted)
}
