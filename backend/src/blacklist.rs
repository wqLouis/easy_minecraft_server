use std::path::Path;

use crate::errors::AppError;

/// Default path for the blacklist file.
pub fn default_blacklist_path() -> std::path::PathBuf {
    std::path::PathBuf::from("./data/blacklist.json")
}

/// Load the blacklist from a JSON array file.
/// Returns an empty vec if the file doesn't exist.
pub fn load_blacklist(path: &Path) -> Result<Vec<String>, AppError> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let ips: Vec<String> = serde_json::from_str(&content)
                .map_err(|e| AppError::Internal(format!("Failed to parse blacklist: {}", e)))?;
            Ok(ips)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(Vec::new())
        }
        Err(e) => Err(AppError::Internal(format!(
            "Failed to read blacklist: {}",
            e
        ))),
    }
}

/// Save the blacklist as a JSON array to the file.
pub fn save_blacklist(path: &Path, ips: &[String]) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("Failed to create data dir: {}", e)))?;
    }
    let content = serde_json::to_string_pretty(ips)
        .map_err(|e| AppError::Internal(format!("Failed to serialize blacklist: {}", e)))?;
    std::fs::write(path, content)
        .map_err(|e| AppError::Internal(format!("Failed to write blacklist: {}", e)))?;
    Ok(())
}
