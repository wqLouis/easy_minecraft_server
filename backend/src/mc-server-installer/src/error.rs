#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error {0} fetching {1}")]
    HttpStatus(u16, String),

    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Software/platform {0} is not supported")]
    Unsupported(String),

    #[error("No version found for {0} {1}")]
    NoVersion(String, String),

    #[error("SHA-1 checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("No stable build available for {0} {1}")]
    NoStableBuild(String, String),

    #[error("Failed to parse version: {0}")]
    VersionParse(String),
}
