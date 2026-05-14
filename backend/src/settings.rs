use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use log::info;

use crate::errors::AppError;
use crate::ip_ban::IpBanManager;

// ---------------------------------------------------------------------------
// Settings structure
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AppSettings {
    /// Max failed auth attempts before an IP is blacklisted permanently.
    pub fail2ban_max_attempts: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            fail2ban_max_attempts: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Settings file path
// ---------------------------------------------------------------------------

pub fn default_settings_path() -> PathBuf {
    PathBuf::from("./data/settings.json")
}

// ---------------------------------------------------------------------------
// Load / save
// ---------------------------------------------------------------------------

pub fn load_settings(path: &PathBuf) -> AppSettings {
    if let Ok(content) = std::fs::read_to_string(path) {
        match serde_json::from_str::<AppSettings>(&content) {
            Ok(s) => {
                info!("Settings loaded from {}", path.display());
                return s;
            }
            Err(e) => {
                log::warn!("Failed to parse {}: {}; using defaults", path.display(), e);
            }
        }
    } else {
        info!("No settings file at {}; using defaults", path.display());
    }
    AppSettings::default()
}

pub fn save_settings(path: &PathBuf, settings: &AppSettings) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::Internal(format!("Failed to create settings dir: {}", e))
        })?;
    }
    let content =
        serde_json::to_string_pretty(settings).map_err(|e| AppError::Internal(e.to_string()))?;
    std::fs::write(path, content)
        .map_err(|e| AppError::Internal(format!("Failed to write settings: {}", e)))?;
    info!("Settings saved to {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Apply settings to a running IpBanManager
// ---------------------------------------------------------------------------

pub fn apply_settings_to_ipban(settings: &AppSettings, ip_ban: &RwLock<IpBanManager>) {
    let mut mgr = ip_ban.write().unwrap();
    mgr.set_max_attempts(settings.fail2ban_max_attempts);
}

// ---------------------------------------------------------------------------
// Update settings at runtime
// ---------------------------------------------------------------------------

pub fn update_settings(
    path: &PathBuf,
    settings: Arc<RwLock<AppSettings>>,
    ip_ban: &RwLock<IpBanManager>,
    new: AppSettings,
) -> Result<AppSettings, AppError> {
    save_settings(path, &new)?;

    {
        let mut s = settings
            .write()
            .map_err(|e| AppError::Internal(format!("Settings lock error: {}", e)))?;
        *s = new.clone();
    }

    apply_settings_to_ipban(&new, ip_ban);

    Ok(new)
}
