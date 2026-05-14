use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Crypto error: {0}")]
    CryptoHash(String),

    #[error("Email already registered")]
    EmailAlreadyExists,

    #[error("API key not found or invalid")]
    ApiKeyNotFound,

    #[error("Missing Authorization header")]
    MissingAuthHeader,

    #[error("Invalid Authorization header format")]
    InvalidAuthHeader,

    #[error("Sudo privileges required")]
    SudoRequired,

    #[error("IP is banned")]
    IpBanned,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("IP not whitelisted for this token")]
    IpNotWhitelisted,
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(err: argon2::password_hash::Error) -> Self {
        AppError::CryptoHash(err.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::EmailAlreadyExists => (StatusCode::CONFLICT, "Email already registered"),
            AppError::ApiKeyNotFound => (StatusCode::UNAUTHORIZED, "API key not found or invalid"),
            AppError::MissingAuthHeader => {
                (StatusCode::UNAUTHORIZED, "Missing Authorization header")
            }
            AppError::InvalidAuthHeader => {
                (StatusCode::UNAUTHORIZED, "Invalid Authorization header format")
            }
            AppError::SudoRequired => (StatusCode::FORBIDDEN, "Sudo privileges required"),
            AppError::IpBanned => (StatusCode::FORBIDDEN, "Your IP has been banned"),
            AppError::InvalidPath(_) => (StatusCode::BAD_REQUEST, "Invalid path"),
            AppError::IpNotWhitelisted => {
                (StatusCode::FORBIDDEN, "This token is not authorized from your IP. Authenticate from your home IP first.")
            }
            AppError::CryptoHash(_) | AppError::Database(_) | AppError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        (status, Json(json!({"error": message}))).into_response()
    }
}
