use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::Utc;
use rand::Rng;
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::errors::AppError;
use crate::ip_ban::IpBanManager;
use crate::models::*;
use crate::settings::AppSettings;

const WHITELIST_TTL_SECS: i64 = 12 * 60 * 60;
use mc_server_manager::registry::ServerRegistry;

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub ip_ban: Arc<RwLock<IpBanManager>>,
    pub settings: Arc<RwLock<AppSettings>>,
    pub settings_path: PathBuf,
    pub blacklist_path: PathBuf,
    pub server_registry: ServerRegistry,
}

impl AppState {
    pub fn get_server(&self, id: &str) -> Result<mc_server_manager::ManagedServer, crate::errors::AppError> {
        self.server_registry.get_server(id)
            .map_err(|e| crate::errors::AppError::Internal(format!("Instance '{id}': {e}")))
    }
}

// ---------------------------------------------------------------------------
// Credential helpers
// ---------------------------------------------------------------------------

/// Generate a cryptographically random token (64 hex chars = 256 bits).
pub fn generate_token() -> String {
    let bytes: [u8; 32] = rand::thread_rng().r#gen();
    hex::encode(bytes)
}

/// Hash `token + username` with argon2id (auto-generated salt embedded in output).
fn hash_credentials(token: &str, username: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let input = format!("{token}{username}");
    Ok(argon2.hash_password(input.as_bytes(), &salt)?.to_string())
}

/// Verify a raw `token + username` against a stored argon2 hash.
fn verify_credentials(token: &str, username: &str, hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash)?;
    let input = format!("{token}{username}");
    Ok(Argon2::default().verify_password(input.as_bytes(), &parsed).is_ok())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/auth/register — create a new user.
/// Returns `{ user: { id, username, is_sudoer, created_at }, api_key }`.
pub async fn register(
    State(state): State<Arc<AppState>>,
    Extension(_requester): Extension<User>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let username = body.username.trim().to_lowercase();
    if username.is_empty() || username.len() < 2 {
        return Err(AppError::Internal("Username must be at least 2 characters".into()));
    }
    if !username.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::Internal("Username can only contain letters, numbers, hyphens and underscores".into()));
    }

    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = ?")
        .bind(&username).fetch_one(&state.db).await?;
    if existing > 0 {
        return Err(AppError::UsernameAlreadyExists);
    }

    let token = generate_token();
    let credential_hash = hash_credentials(&token, &username)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    sqlx::query(
        "INSERT INTO users (id, username, api_key_hash, is_sudoer, created_at, updated_at) VALUES (?, ?, ?, 0, ?, ?)",
    )
    .bind(&id).bind(&username).bind(&credential_hash).bind(&now).bind(&now)
    .execute(&state.db).await?;

    let user = User { id, username: username.clone(), api_key_hash: credential_hash, is_sudoer: false, created_at: now.clone(), updated_at: now };

    Ok((StatusCode::CREATED, Json(json!(CreatedUserResponse {
        user: UserResponse::from(user),
        api_key: token, // the `api_key` field is actually the token
    }))))
}

/// GET /api/auth/me
pub async fn me(Extension(user): Extension<User>) -> impl IntoResponse {
    Json(MeResponse { user: UserResponse::from(user) })
}

// ═══════════════════════════════════════════════════════════════════
// User Management (sudo)
// ═══════════════════════════════════════════════════════════════════

/// GET /api/users — list all users.
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at ASC")
        .fetch_all(&state.db).await?;
    Ok(Json(users.into_iter().map(UserResponse::from).collect()))
}

/// DELETE /api/users/:id — delete a user (cannot delete yourself).
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(requester): Extension<User>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if requester.id == id {
        return Err(AppError::Internal("Cannot delete your own account".into()));
    }
    let result = sqlx::query("DELETE FROM users WHERE id = ?").bind(&id).execute(&state.db).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Internal(format!("User '{id}' not found")));
    }
    Ok((StatusCode::OK, Json(json!({ "deleted": true, "id": id }))))
}

/// PUT /api/users/:id — update a user's username.
#[derive(serde::Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Extension(_requester): Extension<User>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(&id).fetch_optional(&state.db).await?
        .ok_or_else(|| AppError::Internal(format!("User '{id}' not found")))?;

    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let new_username = body.username.unwrap_or_else(|| existing.username.clone());

    sqlx::query("UPDATE users SET username = ?, updated_at = ? WHERE id = ?")
        .bind(&new_username).bind(&now).bind(&id)
        .execute(&state.db).await?;

    let updated = User { username: new_username, updated_at: now, ..existing };
    Ok(Json(UserResponse::from(updated)))
}

// ---------------------------------------------------------------------------
// Credential extraction from Authorization header
// ---------------------------------------------------------------------------

/// Extract `(username, token)` from `Authorization: Bearer <username>:<token>`.
pub fn extract_credentials(headers: &axum::http::HeaderMap) -> Result<(String, String), AppError> {
    let auth = headers.get("Authorization")
        .ok_or(AppError::MissingAuthHeader)?
        .to_str().map_err(|_| AppError::InvalidAuthHeader)?;

    let payload = auth.strip_prefix("Bearer ")
        .ok_or(AppError::InvalidAuthHeader)?.trim();

    let (username, token) = payload.split_once(':')
        .ok_or(AppError::InvalidAuthHeader)?;

    let username = username.trim();
    let token = token.trim();
    if username.is_empty() || token.is_empty() {
        return Err(AppError::InvalidAuthHeader);
    }

    Ok((username.to_string(), token.to_string()))
}

// ---------------------------------------------------------------------------
// User resolution from credentials
// ---------------------------------------------------------------------------

/// Look up a user by `username` and verify `token` against stored hash.
pub async fn resolve_user(
    db: &SqlitePool,
    username: &str,
    token: &str,
) -> Result<User, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::ApiKeyNotFound)?;

    if verify_credentials(token, username, &user.api_key_hash)? {
        Ok(user)
    } else {
        Err(AppError::ApiKeyNotFound)
    }
}

// ---------------------------------------------------------------------------
// Client IP extraction
// ---------------------------------------------------------------------------

pub fn client_ip(req: &axum::http::Request<axum::body::Body>) -> Option<String> {
    if let Some(val) = req.headers().get("x-forwarded-for") {
        if let Ok(s) = val.to_str() {
            if let Some(ip) = s.split(',').next() {
                let ip = ip.trim().to_string();
                if !ip.is_empty() { return Some(ip); }
            }
        }
    }
    if let Some(addr) = req.extensions().get::<std::net::SocketAddr>() {
        return Some(addr.ip().to_string());
    }
    None
}

// ---------------------------------------------------------------------------
// IP whitelist enforcement (sliding 12h window)
// ---------------------------------------------------------------------------

pub async fn check_ip_whitelist(
    db: &SqlitePool, user_id: &str, ip: &str, enabled: bool,
) -> Result<(), AppError> {
    if !enabled { return Ok(()); }

    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let cutoff = (chrono::Utc::now() - chrono::Duration::seconds(WHITELIST_TTL_SECS))
        .format("%Y-%m-%d %H:%M:%S").to_string();

    sqlx::query("DELETE FROM ip_whitelist WHERE user_id = ? AND updated_at < ?")
        .bind(user_id).bind(&cutoff).execute(db).await?;

    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ip_whitelist WHERE user_id = ? AND ip = ? AND updated_at >= ?",
    ).bind(user_id).bind(ip).bind(&cutoff).fetch_one(db).await?;

    if existing > 0 {
        sqlx::query("UPDATE ip_whitelist SET updated_at = ? WHERE user_id = ? AND ip = ?")
            .bind(&now).bind(user_id).bind(ip).execute(db).await?;
        return Ok(());
    }

    let any_active = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ip_whitelist WHERE user_id = ? AND updated_at >= ?",
    ).bind(user_id).bind(&cutoff).fetch_one(db).await?;

    if any_active == 0 {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ip_whitelist (id, user_id, ip, updated_at) VALUES (?, ?, ?, ?)")
            .bind(&id).bind(user_id).bind(ip).bind(&now).execute(db).await?;
        log::info!("IP {ip} auto-registered in whitelist for user {}", &user_id[..8]);
        return Ok(());
    }

    Err(AppError::IpNotWhitelisted)
}
