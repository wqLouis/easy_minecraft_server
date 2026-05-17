//! API error types and HTTP response conversion.
use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Missing Authorization header")]
    MissingAuthHeader,
    #[error("Invalid Authorization header format")]
    InvalidAuthHeader,
    #[error("API key or username not found")]
    ApiKeyNotFound,
    #[error("Sudo privileges required")]
    SudoRequired,
    #[error("IP is banned")]
    IpBanned,
    #[error("IP not whitelisted")]
    IpNotWhitelisted,
    #[error("Username already exists")]
    UsernameAlreadyExists,
    #[error("{0}")]
    Internal(String),
    #[error("Request replay detected")]
    ReplayDetected,
    #[error("Request timestamp expired or missing")]
    TimestampExpired,
    #[error("API key has expired")]
    ApiKeyExpired,
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("{0}")]
    Validation(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            Self::MissingAuthHeader | Self::InvalidAuthHeader | Self::ApiKeyNotFound => {
                (StatusCode::UNAUTHORIZED, self.to_string())
            }
            Self::SudoRequired => (StatusCode::FORBIDDEN, self.to_string()),
            Self::IpBanned => (StatusCode::FORBIDDEN, self.to_string()),
            Self::IpNotWhitelisted => (StatusCode::FORBIDDEN, self.to_string()),
            Self::UsernameAlreadyExists => (StatusCode::CONFLICT, self.to_string()),
            Self::ReplayDetected => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            Self::TimestampExpired => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::ApiKeyExpired => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::InvalidPath(_) | Self::Validation(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into()),
        };
        (status, Json(json!({"error": msg, "code": status.as_u16()}))).into_response()
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(e: argon2::password_hash::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<Box<dyn std::error::Error>> for AppError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        AppError::Internal(e.to_string())
    }
}
