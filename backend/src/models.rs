//! Database models and API response types.
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String, pub username: String, pub api_key_hash: String, pub is_sudoer: bool, pub created_at: String, pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IpWhitelistEntry {
    pub id: String, pub user_id: String, pub ip: String, pub updated_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse { pub id: String, pub username: String, pub is_sudoer: bool, pub created_at: String }
impl From<User> for UserResponse { fn from(u: User) -> Self { Self { id: u.id, username: u.username, is_sudoer: u.is_sudoer, created_at: u.created_at } } }
#[derive(Debug, Serialize, Deserialize)]
pub struct MeResponse { pub user: UserResponse }
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatedUserResponse { pub user: UserResponse, pub api_key: String }
#[derive(Debug, Deserialize)]
pub struct RegisterRequest { pub username: String }
