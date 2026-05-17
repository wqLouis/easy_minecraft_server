//! IP-based rate limiting and permanent blacklisting.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpBanManager {
    #[serde(skip)]
    failures: HashMap<String, u32>,
    blacklist: Vec<String>,
    max_attempts: u32,
}
impl IpBanManager {
    pub fn new(max_attempts: u32) -> Self {
        Self {
            failures: HashMap::new(),
            blacklist: Vec::new(),
            max_attempts,
        }
    }
    pub fn set_max_attempts(&mut self, n: u32) {
        self.max_attempts = n;
    }
    pub fn record_failure(&mut self, ip: &str) -> bool {
        let c = self.failures.entry(ip.to_string()).or_insert(0);
        *c += 1;
        if *c >= self.max_attempts {
            self.blacklist.push(ip.to_string());
            self.failures.remove(ip);
            true
        } else {
            false
        }
    }
    pub fn clear_failures(&mut self, ip: &str) {
        self.failures.remove(ip);
    }
    pub fn is_banned(&self, ip: &str) -> bool {
        self.blacklist.contains(&ip.to_string())
    }
    pub fn add_blacklist(&mut self, ip: &str) {
        if !self.blacklist.contains(&ip.to_string()) {
            self.blacklist.push(ip.to_string());
        }
    }
    pub fn unban(&mut self, ip: &str) -> bool {
        let before = self.blacklist.len();
        self.blacklist.retain(|x| x != ip);
        before != self.blacklist.len()
    }
    pub fn blacklist(&self) -> &[String] {
        &self.blacklist
    }
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({"blacklisted_ips": self.blacklist.len(), "max_attempts": self.max_attempts})
    }
}

pub fn default_blacklist_path() -> PathBuf {
    PathBuf::from("./data/blacklist.json")
}
pub fn load_blacklist(path: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(std::fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default())
}
pub fn save_blacklist(path: &PathBuf, ips: &Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(std::fs::write(path, serde_json::to_string_pretty(ips)?)?)
}
