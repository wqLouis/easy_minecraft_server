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
}

// ---------------------------------------------------------------------------
// API key helpers
// ---------------------------------------------------------------------------

/// Generate a cryptographically random API key (64 hex chars = 256 bits).
pub fn generate_api_key() -> String {
    let bytes: [u8; 32] = rand::thread_rng().r#gen();
    hex::encode(bytes)
}

/// Hash an API key with argon2 for secure DB storage.
fn hash_api_key(api_key: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(api_key.as_bytes(), &salt)?
        .to_string();
    Ok(hash)
}

/// Verify a raw API key against a stored argon2 hash.
fn verify_api_key(api_key: &str, hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(api_key.as_bytes(), &parsed)
        .is_ok())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/auth/register
///
/// Creates a new regular user with a generated API key.
/// Requires sudo privileges (enforced by middleware).
pub async fn register(
    State(state): State<Arc<AppState>>,
    Extension(requester): Extension<User>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !requester.is_sudoer {
        return Err(AppError::SudoRequired);
    }

    let email = body.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::Internal("Invalid email address".to_string()));
    }

    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&state.db)
        .await?;

    if existing > 0 {
        return Err(AppError::EmailAlreadyExists);
    }

    let id = Uuid::new_v4().to_string();
    let api_key = generate_api_key();
    let api_key_hash = hash_api_key(&api_key)?;
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    sqlx::query(
        "INSERT INTO users (id, email, api_key_hash, is_sudoer, created_at, updated_at) \
         VALUES (?, ?, ?, 0, ?, ?)",
    )
    .bind(&id)
    .bind(&email)
    .bind(&api_key_hash)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await?;

    let user = User {
        id,
        email,
        api_key_hash,
        is_sudoer: false,
        created_at: now.clone(),
        updated_at: now,
    };

    Ok((
        StatusCode::CREATED,
        Json(json!(CreatedUserResponse {
            user: UserResponse::from(user),
            api_key,
        })),
    ))
}

/// GET /api/auth/me
pub async fn me(Extension(user): Extension<User>) -> impl IntoResponse {
    Json(MeResponse {
        user: UserResponse::from(user),
    })
}

// ---------------------------------------------------------------------------
// Bearer token extraction
// ---------------------------------------------------------------------------

pub fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Result<String, AppError> {
    let auth_header = headers
        .get("Authorization")
        .ok_or(AppError::MissingAuthHeader)?
        .to_str()
        .map_err(|_| AppError::InvalidAuthHeader)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::InvalidAuthHeader)?
        .trim();

    if token.is_empty() {
        return Err(AppError::InvalidAuthHeader);
    }

    Ok(token.to_string())
}

// ---------------------------------------------------------------------------
// User resolution from API key
// ---------------------------------------------------------------------------

/// Look up a user by their raw API key (bearer token).
/// Iterates all users and tries argon2 verification on each.
pub async fn resolve_user_from_api_key(
    db: &SqlitePool,
    api_key: &str,
) -> Result<User, AppError> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users")
        .fetch_all(db)
        .await?;

    for user in users {
        if verify_api_key(api_key, &user.api_key_hash)? {
            return Ok(user);
        }
    }

    Err(AppError::ApiKeyNotFound)
}

// ---------------------------------------------------------------------------
// Client IP extraction
// ---------------------------------------------------------------------------

/// Extract client IP from request — checks X-Forwarded-For header first,
/// then falls back to the SocketAddr from extensions.
pub fn client_ip(req: &axum::http::Request<axum::body::Body>) -> Option<String> {
    if let Some(val) = req.headers().get("x-forwarded-for") {
        if let Ok(s) = val.to_str() {
            if let Some(ip) = s.split(',').next() {
                let ip = ip.trim().to_string();
                if !ip.is_empty() {
                    return Some(ip);
                }
            }
        }
    }
    if let Some(addr) = req.extensions().get::<std::net::SocketAddr>() {
        return Some(addr.ip().to_string());
    }
    None
}
