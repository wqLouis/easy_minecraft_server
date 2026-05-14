use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ---------------------------------------------------------------------------
// User
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub api_key_hash: String,
    pub is_sudoer: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Public-facing user info (no api key hash)
#[derive(Debug, Clone, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub is_sudoer: bool,
    pub created_at: String,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            is_sudoer: u.is_sudoer,
            created_at: u.created_at,
        }
    }
}

// ---------------------------------------------------------------------------
// API key response — returned once when user is created
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CreatedUserResponse {
    pub user: UserResponse,
    pub api_key: String,
}

// ---------------------------------------------------------------------------
// Request payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user: UserResponse,
}

// ---------------------------------------------------------------------------
// IP Whitelist
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IpWhitelistEntry {
    pub id: String,
    pub user_id: String,
    pub ip: String,
    pub updated_at: String,
}
