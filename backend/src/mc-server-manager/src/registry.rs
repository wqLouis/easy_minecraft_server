//! Server instance registry — manages multiple [`ManagedServer`] instances,
//! persists their configurations to disk, and provides thread-safe access
//! for the API layer.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::instance::ServerConfig;
use crate::manager::{ManagedServer, ServerHandle};

// ---------------------------------------------------------------------------
// Instance configuration
// ---------------------------------------------------------------------------

/// Describes how to create a managed server instance.
///
/// `server_dir` and `jar_path` are filled server-side from settings
/// and provider/version — users should **not** send them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub id: String,
    pub name: String,
    /// Software provider (e.g. "vanilla", "paper", "fabric").
    pub provider: String,
    /// Minecraft version (e.g. "1.21.4").
    pub version: String,
    /// Path to the Java executable.
    pub java_path: String,
    pub min_memory: String,
    pub max_memory: String,
    pub jvm_args: Vec<String>,
    /// Working directory — derived from `{settings.servers_dir}/{id}`.
    #[serde(default)]
    pub server_dir: String,
    /// Local path to the server JAR — derived from `{server_dir}/server.jar`.
    #[serde(default)]
    pub jar_path: String,
}

/// Summary returned when listing instances.
#[derive(Debug, Clone, Serialize)]
pub struct InstanceSummary {
    pub id: String,
    pub name: String,
    pub running: bool,
}

// ---------------------------------------------------------------------------
// Persisted config file format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedConfigs {
    instances: Vec<InstanceConfig>,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Thread-safe registry of managed server instances.
///
/// Configs are persisted to a JSON file so they survive restarts.
#[derive(Clone)]
pub struct ServerRegistry {
    instances: Arc<RwLock<HashMap<String, ManagedServer>>>,
    config_path: PathBuf,
}

impl ServerRegistry {
    /// Create a new registry and load saved configs from `config_path`.
    ///
    /// If the file does not exist an empty registry is returned.
    pub fn new(config_path: PathBuf) -> Self {
        let instances = Arc::new(RwLock::new(HashMap::new()));
        let registry = Self {
            instances: instances.clone(),
            config_path,
        };

        // Load previously saved configs
        if let Ok(saved) = registry.load_configs() {
            let mut map = instances.write().unwrap();
            for cfg in saved.instances {
                let server = cfg.to_managed_server();
                map.insert(cfg.id, server);
            }
        }

        registry
    }

    // ── CRUD ───────────────────────────────────────────────────────

    /// Create a new server instance from a config.
    pub fn create(&self, config: InstanceConfig) -> Result<(), Error> {
        let server = config.to_managed_server();

        let mut instances = self
            .instances
            .write()
            .map_err(|e| Error::other(format!("Registry lock: {e}")))?;

        if instances.contains_key(&config.id) {
            return Err(Error::other(format!(
                "Instance '{}' already exists",
                config.id
            )));
        }

        instances.insert(config.id.clone(), server);
        drop(instances);
        self.save_configs()?;
        Ok(())
    }

    /// Remove a server instance (stop it first if running).
    pub fn remove(&self, id: &str) -> Result<(), Error> {
        let mut instances = self
            .instances
            .write()
            .map_err(|e| Error::other(format!("Registry lock: {e}")))?;

        instances
            .remove(id)
            .ok_or_else(|| Error::other(format!("Instance '{id}' not found")))?;

        drop(instances);
        self.save_configs()?;
        Ok(())
    }

    /// Update an instance's configuration (requires a restart to take effect).
    pub fn update_config(&self, id: &str, config: InstanceConfig) -> Result<(), Error> {
        let new_server = config.to_managed_server();

        let mut instances = self
            .instances
            .write()
            .map_err(|e| Error::other(format!("Registry lock: {e}")))?;

        if !instances.contains_key(id) {
            return Err(Error::other(format!("Instance '{id}' not found")));
        }

        instances.remove(id);
        instances.insert(config.id.clone(), new_server);
        drop(instances);
        self.save_configs()?;
        Ok(())
    }

    // ── Queries ────────────────────────────────────────────────────

    /// Return a summary of all instances.
    pub fn list(&self) -> Vec<InstanceSummary> {
        let instances = match self.instances.read() {
            Ok(map) => map,
            Err(_) => return vec![],
        };

        instances
            .iter()
            .map(|(id, s)| InstanceSummary {
                id: id.clone(),
                name: s.handle().name().to_string(),
                running: s.handle().is_running(),
            })
            .collect()
    }

    /// Get a clone of a managed server by ID.
    ///
    /// Returns `None` if the ID does not exist.
    pub fn get_server(&self, id: &str) -> Result<ManagedServer, Error> {
        let instances = self
            .instances
            .read()
            .map_err(|e| Error::other(format!("Registry lock: {e}")))?;

        instances
            .get(id)
            .cloned()
            .ok_or_else(|| Error::other(format!("Instance '{id}' not found")))
    }

    /// Get the config and handle for an instance (for the API detail endpoint).
    pub fn get_info(&self, id: &str) -> Option<(InstanceConfig, ServerHandle)> {
        let instances = self.instances.read().ok()?;
        let server = instances.get(id)?;
        let config = InstanceConfig::from_managed(server);
        Some((config, server.handle().clone()))
    }

    // ── Lifecycle ──────────────────────────────────────────────────

    /// Start a server instance.
    pub async fn start(&self, id: &str) -> Result<(), Error> {
        let mut server = self.get_server(id)?;
        server.start().await?;
        // Re-insert so config changes (e.g. installer-updated jar_path) persist
        {
            let mut instances = self
                .instances
                .write()
                .map_err(|e| Error::other(format!("Registry lock: {e}")))?;
            instances.insert(id.to_string(), server);
        }
        self.save_configs()
    }

    /// Stop a server instance.
    pub async fn stop(&self, id: &str) -> Result<(), Error> {
        let server = self.get_server(id)?;
        server.stop().await
    }

    /// Force-kill a server instance.
    pub async fn kill(&self, id: &str) -> Result<(), Error> {
        let server = self.get_server(id)?;
        server.kill().await
    }

    /// Send a console command to a running server.
    pub fn send_command(&self, id: &str, cmd: &str) -> Result<(), Error> {
        let server = self.get_server(id)?;
        server.send_command(cmd)
    }

    // ── Persistence ────────────────────────────────────────────────

    fn load_configs(&self) -> Result<SavedConfigs, Error> {
        let content = match std::fs::read_to_string(&self.config_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SavedConfigs {
                    instances: Vec::new(),
                });
            }
            Err(e) => {
                return Err(Error::other(format!(
                    "Failed to read {}: {e}",
                    self.config_path.display()
                )));
            }
        };

        serde_json::from_str(&content).map_err(|e| {
            Error::other(format!(
                "Failed to parse {}: {e}",
                self.config_path.display()
            ))
        })
    }

    fn save_configs(&self) -> Result<(), Error> {
        let instances = self
            .instances
            .read()
            .map_err(|e| Error::other(format!("Registry lock: {e}")))?;

        let configs: Vec<InstanceConfig> = instances
            .values()
            .map(|s| InstanceConfig::from_managed(s))
            .collect();

        drop(instances);

        let saved = SavedConfigs { instances: configs };
        let content = serde_json::to_string_pretty(&saved)
            .map_err(|e| Error::other(format!("Serialization error: {e}")))?;

        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::other(format!("Failed to create dir: {e}")))?;
        }

        std::fs::write(&self.config_path, content)
            .map_err(|e| Error::other(format!("Failed to write {}: {e}", self.config_path.display())))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl InstanceConfig {
    fn to_managed_server(&self) -> ManagedServer {
        let server_config = ServerConfig::new(
            &self.jar_path,
            &self.java_path,
            &self.min_memory,
            &self.max_memory,
            &self.server_dir,
        )
        .with_jvm_args(self.jvm_args.clone());

        ManagedServer::new(
            self.id.clone(),
            self.name.clone(),
            server_config,
            PathBuf::from(&self.server_dir).join("data"),
            self.provider.clone(),
            self.version.clone(),
        )
    }

    fn from_managed(server: &ManagedServer) -> Self {
        let cfg = server.config();
        Self {
            id: server.handle().id().to_string(),
            name: server.handle().name().to_string(),
            provider: server.provider().to_string(),
            version: server.version().to_string(),
            jar_path: cfg.jar_path.to_string_lossy().to_string(),
            java_path: cfg.java_path.to_string_lossy().to_string(),
            min_memory: cfg.min_memory.clone(),
            max_memory: cfg.max_memory.clone(),
            server_dir: cfg.server_dir.to_string_lossy().to_string(),
            jvm_args: cfg.jvm_args.clone(),
        }
    }
}
