//! Minecraft `server.properties` file management.
//!
//! [`ServerProperties`] provides read, write, and in-mutation of the
//! standard `server.properties` configuration file that every Minecraft
//! server uses at startup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::Error;

/// Represents a parsed `server.properties` file.
///
/// # Example
///
/// ```rust,no_run
/// use mc_server_manager::ServerProperties;
///
/// let mut props = ServerProperties::load("/srv/minecraft/server.properties").unwrap();
///
/// // Read a value
/// if let Some(port) = props.get("server-port") {
///     println!("Server port: {port}");
/// }
///
/// // Update a value
/// props.set("max-players".into(), "20".into());
/// props.set("difficulty".into(), "hard".into());
///
/// // Persist to disk
/// props.save().unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct ServerProperties {
    /// Key-value pairs parsed from the file.
    properties: HashMap<String, String>,
    /// Path to the `server.properties` file on disk.
    path: PathBuf,
    /// Comments and blank lines from the original file (preserved on save).
    preamble: Vec<String>,
}

impl ServerProperties {
    /// Load properties from a `server.properties` file.
    ///
    /// If the file doesn't exist, an empty set is returned (so you can
    /// create defaults and save).
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    properties: HashMap::new(),
                    path,
                    preamble: Vec::new(),
                });
            }
            Err(e) => {
                return Err(Error::PropertiesLoad {
                    path,
                    source: e,
                });
            }
        };

        let mut properties = HashMap::new();
        let mut preamble = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                preamble.push(line.to_string());
                continue;
            }

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim().to_string();
                let value = trimmed[eq_pos + 1..].trim().to_string();
                properties.insert(key, value);
            } else {
                preamble.push(line.to_string()); // preserve unknown lines
            }
        }

        Ok(Self {
            properties,
            path,
            preamble,
        })
    }

    /// Save the current properties back to disk, preserving the original
    /// preamble (comments).
    pub fn save(&self) -> Result<(), Error> {
        let path = &self.path;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::PropertiesSave {
                path: path.clone(),
                source: e,
            })?;
        }

        let mut content = String::new();

        // Write preamble (comments from original file)
        for line in &self.preamble {
            content.push_str(line);
            content.push('\n');
        }

        // Write a separator comment if we have both preamble and properties
        if !self.preamble.is_empty() && !self.properties.is_empty() {
            content.push('\n');
        }

        // Write key=value pairs sorted by key for reproducibility
        let mut keys: Vec<&String> = self.properties.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(value) = self.properties.get(key) {
                content.push_str(&format!("{}={}\n", key, value));
            }
        }

        std::fs::write(path, content).map_err(|e| Error::PropertiesSave {
            path: path.clone(),
            source: e,
        })?;

        log::info!("Saved {} properties to {}", self.properties.len(), path.display());
        Ok(())
    }

    // ── Getters / Setters ──────────────────────────────────────────

    /// Get a property value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    /// Set a property value. If the key already exists it's updated.
    pub fn set(&mut self, key: String, value: String) {
        self.properties.insert(key, value);
    }

    /// Remove a property. Returns the old value if it existed.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.properties.remove(key)
    }

    /// Returns `true` if the property exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }

    /// Return the number of properties.
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Returns `true` if no properties have been loaded.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Return an immutable reference to all properties.
    pub fn all(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// Return the path of the properties file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    // ── Defaults ───────────────────────────────────────────────────

    /// Return a set of sensible default properties for a standard
    /// Minecraft server.
    pub fn default_map() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("enable-jmx-monitoring".into(), "false".into());
        m.insert("rcon.port".into(), "25575".into());
        m.insert("level-seed".into(), "".into());
        m.insert("gamemode".into(), "survival".into());
        m.insert("enable-command-block".into(), "false".into());
        m.insert("enable-query".into(), "false".into());
        m.insert("generator-settings".into(), "".into());
        m.insert("level-name".into(), "world".into());
        m.insert("motd".into(), "A Minecraft Server".into());
        m.insert("query.port".into(), "25565".into());
        m.insert("pvp".into(), "true".into());
        m.insert("generate-structures".into(), "true".into());
        m.insert("difficulty".into(), "easy".into());
        m.insert("network-compression-threshold".into(), "256".into());
        m.insert("max-tick-time".into(), "60000".into());
        m.insert("max-players".into(), "20".into());
        m.insert("online-mode".into(), "true".into());
        m.insert("allow-nether".into(), "true".into());
        m.insert("banned-ips".into(), "".into());
        m.insert("enforce-whitelist".into(), "false".into());
        m.insert("server-port".into(), "25565".into());
        m.insert("enable-rcon".into(), "false".into());
        m.insert("sync-chunk-writes".into(), "true".into());
        m.insert("op-permission-level".into(), "4".into());
        m.insert("prevent-proxy-connections".into(), "false".into());
        m.insert("hide-online-players".into(), "false".into());
        m.insert("resource-pack".into(), "".into());
        m.insert("entity-broadcast-range-percentage".into(), "100".into());
        m.insert("simulation-distance".into(), "10".into());
        m.insert("rcon.password".into(), "".into());
        m.insert("player-idle-timeout".into(), "0".into());
        m.insert("debug".into(), "false".into());
        m.insert("force-gamemode".into(), "false".into());
        m.insert("rate-limit".into(), "0".into());
        m.insert("hardcore".into(), "false".into());
        m.insert("white-list".into(), "false".into());
        m.insert("broadcast-rcon-to-ops".into(), "true".into());
        m.insert("enable-dynmap".into(), "false".into());
        m.insert("spawn-protection".into(), "16".into());
        m.insert("enable-jmx-monitoring".into(), "false".into());
        m.insert("view-distance".into(), "10".into());
        m.insert("max-world-size".into(), "29999984".into());
        m.insert("function-permission-level".into(), "2".into());
        m.insert("broadcast-console-to-ops".into(), "true".into());
        m.insert("allow-flight".into(), "false".into());
        m.insert("text-filtering-config".into(), "".into());
        m.insert("enforce-secure-profile".into(), "true".into());
        m.insert("max-chained-neighbor-updates".into(), "1000000".into());
        m
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_format() {
        let content = "#Minecraft server properties\n#Thu Jan 01 00:00:00 UTC 1970\n\ngamemode=survival\nserver-port=25565\n";
        let dir = std::env::temp_dir().join("mc_props_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("server.properties");
        std::fs::write(&path, content).unwrap();

        let props = ServerProperties::load(&path).unwrap();
        assert_eq!(props.get("gamemode"), Some("survival"));
        assert_eq!(props.get("server-port"), Some("25565"));
        assert_eq!(props.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_set_and_save() {
        let dir = std::env::temp_dir().join("mc_props_test2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("server.properties");

        let mut props = ServerProperties::load(&path).unwrap();
        props.set("max-players".into(), "10".into());
        props.set("difficulty".into(), "hard".into());
        props.save().unwrap();

        let loaded = ServerProperties::load(&path).unwrap();
        assert_eq!(loaded.get("max-players"), Some("10"));
        assert_eq!(loaded.get("difficulty"), Some("hard"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
