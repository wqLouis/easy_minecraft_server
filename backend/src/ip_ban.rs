use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::errors::AppError;

/// Tracks authentication failures per IP.
///
/// When `max_attempts` is exceeded the IP is added to the **permanent
/// blacklist** and stays there until a sudoer explicitly removes it.
#[derive(Debug, Clone)]
pub struct IpBanManager {
    /// IP → timestamps of recent failed auths (sliding window)
    failed_attempts: HashMap<String, Vec<Instant>>,
    /// Permanent blacklist (mirrors the list in settings.json)
    blacklist: Vec<String>,

    max_attempts: u32,
    /// Window (seconds) in which failures count toward a ban.
    window_secs: u64,
}

impl IpBanManager {
    pub fn new(max_attempts: u32) -> Self {
        Self {
            failed_attempts: HashMap::new(),
            blacklist: Vec::new(),
            max_attempts,
            window_secs: 600, // 10 minute sliding window
        }
    }

    // ── Configuration ──────────────────────────────────────────────

    pub fn set_max_attempts(&mut self, n: u32) {
        self.max_attempts = n;
    }

    #[allow(dead_code)]
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    // ── Blacklist management ───────────────────────────────────────

    #[allow(dead_code)]
    pub fn blacklist(&self) -> &[String] {
        &self.blacklist
    }

    /// Add an IP to the blacklist (e.g. restored from settings).
    pub fn add_blacklist(&mut self, ip: &str) {
        let ip = ip.trim().to_string();
        if !self.blacklist.contains(&ip) {
            self.blacklist.push(ip);
        }
    }

    /// Remove an IP from the blacklist. Returns `true` if it was present.
    pub fn remove_blacklist(&mut self, ip: &str) -> bool {
        let len = self.blacklist.len();
        self.blacklist.retain(|x| x != ip);
        self.blacklist.len() < len
    }

    // ── Fail2ban ───────────────────────────────────────────────────

    /// Check whether this IP is currently blacklisted.
    pub fn is_banned(&self, ip: &str) -> bool {
        self.blacklist.contains(&ip.to_string())
    }

    /// Record a failed auth from this IP.
    ///
    /// Returns `true` **iff** the IP was *just now* added to the blacklist
    /// as a result of crossing the failure threshold.
    pub fn record_failure(&mut self, ip: &str) -> bool {
        let now = Instant::now();
        let entry = self.failed_attempts.entry(ip.to_string()).or_default();
        entry.push(now);

        // Trim old entries outside the window
        let cutoff = now - Duration::from_secs(self.window_secs);
        entry.retain(|t| *t > cutoff);

        // Check threshold
        if entry.len() as u32 >= self.max_attempts {
            // Add to permanent blacklist
            self.add_blacklist(ip);
            self.failed_attempts.remove(ip);
            return true; // newly blacklisted
        }

        false
    }

    /// Clear failure history (e.g. on successful auth).
    pub fn clear_failures(&mut self, ip: &str) {
        self.failed_attempts.remove(ip);
    }

    /// Unban: remove from blacklist entirely.
    pub fn unban(&mut self, ip: &str) -> bool {
        self.remove_blacklist(ip)
    }

    // ── Status ─────────────────────────────────────────────────────

    pub fn status(&self) -> BanStatus {
        BanStatus {
            blacklisted_ips: self.blacklist.clone(),
            max_attempts: self.max_attempts,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BanStatus {
    pub blacklisted_ips: Vec<String>,
    pub max_attempts: u32,
}

// ── Blacklist persistence ──────────────────────────────────────────

/// Default path for the blacklist file.
pub fn default_blacklist_path() -> std::path::PathBuf {
    std::path::PathBuf::from("./data/blacklist.json")
}

/// Load the blacklist from a JSON array file.
pub fn load_blacklist(path: &Path) -> Result<Vec<String>, AppError> {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map_err(|e| AppError::Internal(format!("Failed to parse blacklist: {e}"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(AppError::Internal(format!("Failed to read blacklist: {e}"))),
    }
}

/// Save the blacklist as a JSON array to the file.
pub fn save_blacklist(path: &Path, ips: &[String]) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("Failed to create dir: {e}")))?;
    }
    let content = serde_json::to_string_pretty(ips)
        .map_err(|e| AppError::Internal(format!("Failed to serialize: {e}")))?;
    std::fs::write(path, content)
        .map_err(|e| AppError::Internal(format!("Failed to write: {e}")))
}
