//! Application settings — load, save, update at runtime.
use crate::errors::AppError;
use crate::ip_ban::IpBanManager;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AppSettings {
    pub fail2ban_max_attempts: u32,
    #[serde(default = "default_servers_dir")]
    pub servers_dir: String,
    #[serde(default = "default_java_path")]
    pub java_path: String,
    #[serde(default)]
    pub ip_whitelist_enabled: bool,
    #[serde(default)]
    pub trust_proxy_headers: bool,
}
fn default_servers_dir() -> String {
    "./servers".into()
}
fn default_java_path() -> String {
    if cfg!(target_os = "windows") {
        "java".into()
    } else {
        "/usr/bin/java".into()
    }
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            fail2ban_max_attempts: 5,
            servers_dir: default_servers_dir(),
            java_path: default_java_path(),
            ip_whitelist_enabled: false,
            trust_proxy_headers: false,
        }
    }
}

pub fn default_settings_path() -> PathBuf {
    PathBuf::from("./data/settings.json")
}

pub fn load_settings(path: &PathBuf) -> AppSettings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| {
            log::info!("No settings at {}, using defaults", path.display());
            AppSettings::default()
        })
}

pub fn save_settings(path: &PathBuf, settings: &AppSettings) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("settings dir: {e}")))?;
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(settings).map_err(|e| AppError::Internal(e.to_string()))?,
    )
    .map_err(|e| AppError::Internal(format!("write settings: {e}")))
}

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
            .map_err(|e| AppError::Internal(format!("Settings lock: {e}")))?;
        *s = new.clone();
    }
    ip_ban
        .write()
        .map_err(|e| AppError::Internal(format!("ipban lock: {e}")))
        .map(|mut m| m.set_max_attempts(new.fail2ban_max_attempts))?;
    Ok(new)
}
