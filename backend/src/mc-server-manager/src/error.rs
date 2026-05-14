//! Error types for the Minecraft server manager.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // ── Process / I/O ─────────────────────────────────────────────
    #[error("Java process exited prematurely (status: {0:?})")]
    PrematureExit(std::process::ExitStatus),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to send command to server stdin")]
    StdinSend,

    // ── Properties ────────────────────────────────────────────────
    #[error("Failed to load server.properties from {path}: {source}")]
    PropertiesLoad {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to save server.properties to {path}: {source}")]
    PropertiesSave {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Invalid property line (malformed key=value): {line}")]
    MalformedProperty { line: String },

    // ── Installer ─────────────────────────────────────────────────
    #[error("Installer error: {0}")]
    Installer(#[from] mc_server_installer::error::Error),

    // ── General ───────────────────────────────────────────────────
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}
