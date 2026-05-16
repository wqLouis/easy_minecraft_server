//! Minecraft `server.properties` file management.
//!
//! Reads and writes the standard `server.properties` file.
//! Comments from the original file are preserved as-is.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct ServerProperties {
    properties: HashMap<String, String>,
    path: PathBuf,
}

impl ServerProperties {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self { properties: HashMap::new(), path });
            }
            Err(e) => return Err(Error::PropertiesLoad { path, source: e }),
        };

        let mut properties = HashMap::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(eq) = trimmed.find('=') {
                if !trimmed.starts_with('#') {
                    properties.insert(trimmed[..eq].trim().to_string(), trimmed[eq + 1..].trim().to_string());
                }
            }
        }

        Ok(Self { properties, path })
    }

    pub fn save(&self) -> Result<(), Error> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::PropertiesSave { path: self.path.clone(), source: e })?;
        }
        let mut content = String::new();
        let mut keys: Vec<&String> = self.properties.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(value) = self.properties.get(key) {
                content.push_str(&format!("{}={}\n", key, value));
            }
        }
        std::fs::write(&self.path, content)
            .map_err(|e| Error::PropertiesSave { path: self.path.clone(), source: e })?;
        log::info!("Saved {} properties to {}", self.properties.len(), self.path.display());
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&str> { self.properties.get(key).map(|s| s.as_str()) }
    pub fn set(&mut self, key: String, value: String) { self.properties.insert(key, value); }
    pub fn remove(&mut self, key: &str) -> Option<String> { self.properties.remove(key) }
    pub fn contains_key(&self, key: &str) -> bool { self.properties.contains_key(key) }
    pub fn len(&self) -> usize { self.properties.len() }
    pub fn is_empty(&self) -> bool { self.properties.is_empty() }
    pub fn all(&self) -> &HashMap<String, String> { &self.properties }
    pub fn path(&self) -> &Path { &self.path }
    /// Re-read the file from disk, merging any new keys into the current map.
    pub fn reload(&mut self) -> Result<(), Error> {
        let content = std::fs::read_to_string(&self.path)?;
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(eq) = trimmed.find('=') {
                if !trimmed.starts_with('#') {
                    self.properties.insert(trimmed[..eq].trim().to_string(), trimmed[eq + 1..].trim().to_string());
                }
            }
        }
        Ok(())
    }
}
