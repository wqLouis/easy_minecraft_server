//! Authentication — credential helpers, handlers, IP whitelist.
use crate::errors::AppError;
use crate::ip_ban::IpBanManager;
use crate::models::*;
use crate::settings::AppSettings;
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Extension, Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::Utc;
use mc_server_manager::registry::ServerRegistry;
use rand::Rng;
use serde_json::json;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const W_TTL: i64 = 12 * 60 * 60;
const REPLAY_WINDOW_SECS: i64 = 30; // ±30s timestamp tolerance

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub ip_ban: Arc<RwLock<IpBanManager>>,
    pub settings: Arc<RwLock<AppSettings>>,
    pub settings_path: PathBuf,
    pub blacklist_path: PathBuf,
    pub server_registry: ServerRegistry,
    pub rate_limiter: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    /// Replay cache: user_id → (nonce → seen_at Instant)
    pub replay_cache: Arc<Mutex<HashMap<String, HashMap<String, Instant>>>>,
    /// Tmpfs root path (set when --tmpfs is used), for validation.
    pub tmpfs_root: Option<PathBuf>,
}
impl AppState {
    pub fn get_server(&self, id: &str) -> Result<mc_server_manager::ManagedServer, AppError> {
        self.server_registry
            .get_server(id)
            .map_err(|e| AppError::Internal(format!("Instance '{id}': {e}")))
    }
}

pub fn generate_token() -> String {
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+[]{}|;:,.<>?/~";
    let mut rng = rand::thread_rng();
    let len = rng.gen_range(40..=64);
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
fn hash_cred(token: &str, username: &str) -> Result<String, AppError> {
    Ok(Argon2::default()
        .hash_password(
            format!("{token}{username}").as_bytes(),
            &SaltString::generate(&mut OsRng),
        )?
        .to_string())
}
fn verify_cred(token: &str, username: &str, hash: &str) -> Result<bool, AppError> {
    Ok(Argon2::default()
        .verify_password(
            format!("{token}{username}").as_bytes(),
            &PasswordHash::new(hash)?,
        )
        .is_ok())
}

/// Extract X-Timestamp (Unix epoch seconds) and X-Nonce from request headers.
pub fn extract_replay_headers(headers: &axum::http::HeaderMap) -> Result<(i64, String), AppError> {
    let ts_str = headers
        .get("x-timestamp")
        .ok_or(AppError::TimestampExpired)?
        .to_str()
        .map_err(|_| AppError::TimestampExpired)?
        .trim();
    let ts: i64 = ts_str.parse().map_err(|_| AppError::TimestampExpired)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    if (now - ts).abs() > REPLAY_WINDOW_SECS {
        return Err(AppError::TimestampExpired);
    }
    let nonce = headers
        .get("x-nonce")
        .ok_or(AppError::ReplayDetected)?
        .to_str()
        .map_err(|_| AppError::ReplayDetected)?
        .trim()
        .to_string();
    if nonce.is_empty() {
        return Err(AppError::ReplayDetected);
    }
    Ok((ts, nonce))
}

/// Check the in-memory nonce cache to detect and prevent replay attacks.
///
/// 1. Prunes entries older than `REPLAY_WINDOW_SECS` from the user's cache.
/// 2. If the nonce already exists → ReplayDetected.
/// 3. Otherwise inserts the nonce and returns Ok.
pub fn check_replay(
    cache: &Arc<Mutex<HashMap<String, HashMap<String, Instant>>>>,
    user_id: &str,
    nonce: &str,
) -> Result<(), AppError> {
    let mut map = cache.lock().unwrap();
    let user_cache = map.entry(user_id.to_string()).or_default();
    // Prune old nonces for this user
    let cutoff = Instant::now() - Duration::from_secs(REPLAY_WINDOW_SECS as u64);
    user_cache.retain(|_, seen_at| *seen_at > cutoff);
    if user_cache.contains_key(nonce) {
        return Err(AppError::ReplayDetected);
    }
    user_cache.insert(nonce.to_string(), Instant::now());
    Ok(())
}

/// Check whether the user's API key has expired.
pub fn check_key_expiry(user: &User) -> Result<(), AppError> {
    if let Some(ref expires_at) = user.api_key_expires_at {
        if let Ok(exp_naive) =
            chrono::NaiveDateTime::parse_from_str(expires_at, "%Y-%m-%d %H:%M:%S")
        {
            let exp_utc: chrono::DateTime<chrono::Utc> =
                chrono::DateTime::from_naive_utc_and_offset(exp_naive, chrono::Utc);
            if chrono::Utc::now() > exp_utc {
                return Err(AppError::ApiKeyExpired);
            }
        }
    }
    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────────
pub async fn register(
    State(s): State<Arc<AppState>>,
    Extension(_r): Extension<User>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let username = body.username.trim().to_lowercase();
    if username.is_empty() || username.len() < 2 {
        return Err(AppError::Internal("Username must be ≥2 chars".into()));
    }
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::Internal("Alphanumeric, -, _ only".into()));
    }
    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = ?")
        .bind(&username)
        .fetch_one(&s.db)
        .await?;
    if existing > 0 {
        return Err(AppError::UsernameAlreadyExists);
    }
    let token = generate_token();
    let hash = hash_cred(&token, &username)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    // Normal users: infinite key (NULL = never expires)
    sqlx::query("INSERT INTO users (id, username, api_key_hash, is_sudoer, created_at, updated_at, api_key_expires_at) VALUES (?, ?, ?, 0, ?, ?, NULL)")
        .bind(&id).bind(&username).bind(&hash).bind(&now).bind(&now)
        .execute(&s.db).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!(CreatedUserResponse {
            user: UserResponse::from(User {
                id,
                username,
                api_key_hash: hash,
                is_sudoer: false,
                created_at: now.clone(),
                updated_at: now,
                api_key_expires_at: None,
            }),
            api_key: token,
            api_key_expires_at: None,
        })),
    ))
}

pub async fn me(Extension(u): Extension<User>) -> impl IntoResponse {
    Json(MeResponse {
        user: UserResponse::from(u),
    })
}

pub async fn list_users(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    Ok(Json(
        sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at ASC")
            .fetch_all(&s.db)
            .await?
            .into_iter()
            .map(UserResponse::from)
            .collect(),
    ))
}

pub async fn delete_user(
    State(s): State<Arc<AppState>>,
    Extension(r): Extension<User>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if r.id == id {
        return Err(AppError::Internal("Cannot delete yourself".into()));
    }
    if sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&id)
        .execute(&s.db)
        .await?
        .rows_affected()
        == 0
    {
        return Err(AppError::Internal(format!("User '{id}' not found")));
    }
    Ok((StatusCode::OK, Json(json!({"deleted": true, "id": id}))))
}

#[derive(serde::Deserialize)]
pub struct UpdateUserReq {
    pub username: Option<String>,
}
pub async fn update_user(
    State(s): State<Arc<AppState>>,
    Extension(_r): Extension<User>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(body): Json<UpdateUserReq>,
) -> Result<Json<UserResponse>, AppError> {
    let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(&id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| AppError::Internal(format!("User '{id}' not found")))?;
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let name = body.username.unwrap_or(existing.username);
    sqlx::query("UPDATE users SET username = ?, updated_at = ? WHERE id = ?")
        .bind(&name)
        .bind(&now)
        .bind(&id)
        .execute(&s.db)
        .await?;
    Ok(Json(UserResponse::from(User {
        username: name,
        updated_at: now,
        ..existing
    })))
}

pub fn extract_credentials(headers: &axum::http::HeaderMap) -> Result<(String, String), AppError> {
    let auth = headers
        .get("Authorization")
        .ok_or(AppError::MissingAuthHeader)?
        .to_str()
        .map_err(|_| AppError::InvalidAuthHeader)?;
    let p = auth
        .strip_prefix("Bearer ")
        .ok_or(AppError::InvalidAuthHeader)?
        .trim();
    let (username, token) = p.split_once(':').ok_or(AppError::InvalidAuthHeader)?;
    let (u, t) = (username.trim(), token.trim());
    if u.is_empty() || t.is_empty() {
        return Err(AppError::InvalidAuthHeader);
    }
    Ok((u.to_string(), t.to_string()))
}

pub async fn resolve_user(db: &SqlitePool, username: &str, token: &str) -> Result<User, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::ApiKeyNotFound)?;
    if !verify_cred(token, username, &user.api_key_hash)? {
        return Err(AppError::ApiKeyNotFound);
    }
    check_key_expiry(&user)?;
    Ok(user)
}

pub fn client_ip(req: &axum::http::Request<axum::body::Body>, trust_proxy: bool) -> Option<String> {
    if trust_proxy {
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
    }
    req.extensions()
        .get::<std::net::SocketAddr>()
        .map(|a| a.ip().to_string())
}

pub async fn check_ip_whitelist(
    db: &SqlitePool,
    uid: &str,
    ip: &str,
    enabled: bool,
) -> Result<(), AppError> {
    if !enabled {
        return Ok(());
    }
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let cutoff = (chrono::Utc::now() - chrono::Duration::seconds(W_TTL))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    sqlx::query("DELETE FROM ip_whitelist WHERE user_id = ? AND updated_at < ?")
        .bind(uid)
        .bind(&cutoff)
        .execute(db)
        .await?;
    let active = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ip_whitelist WHERE user_id = ? AND ip = ? AND updated_at >= ?",
    )
    .bind(uid)
    .bind(ip)
    .bind(&cutoff)
    .fetch_one(db)
    .await?;
    if active > 0 {
        sqlx::query("UPDATE ip_whitelist SET updated_at = ? WHERE user_id = ? AND ip = ?")
            .bind(&now)
            .bind(uid)
            .bind(ip)
            .execute(db)
            .await?;
        return Ok(());
    }
    let any = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ip_whitelist WHERE user_id = ? AND updated_at >= ?",
    )
    .bind(uid)
    .bind(&cutoff)
    .fetch_one(db)
    .await?;
    if any == 0 {
        sqlx::query("INSERT INTO ip_whitelist (id, user_id, ip, updated_at) VALUES (?, ?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(uid)
            .bind(ip)
            .bind(&now)
            .execute(db)
            .await?;
        return Ok(());
    }
    Err(AppError::IpNotWhitelisted)
}
