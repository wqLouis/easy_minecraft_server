//! Server instance registry — manages multiple [`ManagedServer`] instances,
//! persists their configurations to disk, and provides thread-safe access
//! for the API layer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::instance::ServerConfig;
use crate::manager::{ManagedServer, ServerHandle};
use crate::world::{dir_size, human_size};

// ---------------------------------------------------------------------------
// Instance configuration
// ---------------------------------------------------------------------------

/// Describes how to create a managed server instance.
///
/// `server_dir` and `jar_path` are filled server-side from settings
/// and provider/version — users should **not** send them.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

/// Summary of an archived instance.
#[derive(Debug, Clone, Serialize)]
pub struct ArchivedSummary {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub version: String,
    pub archived_at: String,
    pub size_bytes: u64,
    pub size_human: String,
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
/// When an instance is removed, its server directory is moved to
/// `_archived/` rather than deleted, so sudoers can restore it later.
#[derive(Clone)]
pub struct ServerRegistry {
    instances: Arc<RwLock<HashMap<String, ManagedServer>>>,
    config_path: PathBuf,
    archive_root: PathBuf,
}

impl ServerRegistry {
    /// Create a new registry and load saved configs from `config_path`.
    ///
    /// If the file does not exist an empty registry is returned.
    pub fn new(config_path: PathBuf) -> Self {
        let instances = Arc::new(RwLock::new(HashMap::new()));

        // Archive root: `_archived/` next to the config file
        let archive_root = config_path
            .parent()
            .map(|p| p.join("_archived"))
            .unwrap_or_else(|| PathBuf::from("./data/_archived"));

        let registry = Self {
            instances: instances.clone(),
            config_path,
            archive_root,
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

    /// Remove (archive) a server instance.
    ///
    /// The server must be stopped first. Its directory is moved to
    /// `_archived/{id}/` so a sudoer can restore it later.
    pub fn remove(&self, id: &str) -> Result<(), Error> {
        // Read the server config *before* removing from the map
        let (config, handle) = self
            .get_info(id)
            .ok_or_else(|| Error::other(format!("Instance '{id}' not found")))?;

        if handle.is_running() {
            return Err(Error::other(
                "Cannot archive a running server. Stop it first.",
            ));
        }

        let src = PathBuf::from(&config.server_dir);
        let dst = self.archive_root.join(id);

        // Move the server directory to archive
        if src.exists() {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| Error::other(format!("Failed to create archive dir: {e}")))?;
            }
            std::fs::rename(&src, &dst).map_err(|e| {
                Error::other(format!(
                    "Failed to archive server directory '{}': {e}",
                    src.display()
                ))
            })?;
        }

        // Save the config manifest inside the archive
        if dst.exists() {
            let manifest_path = dst.join(".instance.json");
            if let Ok(content) = serde_json::to_string_pretty(&config) {
                let _ = std::fs::write(&manifest_path, content);
            }
        }

        // Remove from the in-memory registry
        {
            let mut instances = self
                .instances
                .write()
                .map_err(|e| Error::other(format!("Registry lock: {e}")))?;
            instances.remove(id);
        }

        self.save_configs()?;
        log::info!(
            "Instance '{}' archived ({} → {})",
            id,
            src.display(),
            dst.display()
        );
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

    // ── Archive / Restore ──────────────────────────────────────────

    /// List all archived instances.
    pub fn list_archived(&self) -> Vec<ArchivedSummary> {
        let mut archived = Vec::new();

        let archive_dir = &self.archive_root;
        if !archive_dir.is_dir() {
            return archived;
        }

        let mut entries = match std::fs::read_dir(archive_dir) {
            Ok(e) => e,
            Err(_) => return archived,
        };

        while let Some(Ok(entry)) = entries.next() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let id = entry.file_name().to_string_lossy().to_string();

            // Try to load the manifest
            let manifest_path = path.join(".instance.json");
            let config: Option<InstanceConfig> = std::fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok());

            let (name, provider, version) = config
                .as_ref()
                .map(|c| (c.name.clone(), c.provider.clone(), c.version.clone()))
                .unwrap_or_else(|| (id.clone(), "unknown".into(), "unknown".into()));

            // Compute directory size
            let size_bytes = dir_size(&path).unwrap_or(0);

            // Get modification time as archive timestamp
            let archived_at = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_default();

            archived.push(ArchivedSummary {
                id,
                name,
                provider,
                version,
                archived_at,
                size_bytes,
                size_human: human_size(size_bytes),
            });
        }

        // Sort newest first
        archived.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
        archived
    }

    /// Restore an archived instance back into the active registry.
    ///
    /// Moves the directory from `_archived/{id}/` back to its original
    /// location (read from `.instance.json` manifest).
    pub fn restore_archived(&self, id: &str) -> Result<(), Error> {
        let archived_dir = self.archive_root.join(id);
        if !archived_dir.is_dir() {
            return Err(Error::other(format!(
                "Archived instance '{id}' not found at {}",
                archived_dir.display()
            )));
        }

        // Read the manifest
        let manifest_path = archived_dir.join(".instance.json");
        let manifest_str = std::fs::read_to_string(&manifest_path)
            .map_err(|e| Error::other(format!("Failed to read manifest: {e}")))?;
        let config: InstanceConfig = serde_json::from_str(&manifest_str)
            .map_err(|e| Error::other(format!("Failed to parse manifest: {e}")))?;

        // Check the original location is free
        let dst = PathBuf::from(&config.server_dir);
        if dst.exists() {
            return Err(Error::other(format!(
                "Target directory already exists: {}",
                dst.display()
            )));
        }

        // Move back
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::other(format!("Failed to create server dir: {e}")))?;
        }
        std::fs::rename(&archived_dir, &dst).map_err(|e| {
            Error::other(format!(
                "Failed to restore server directory: {e}"
            ))
        })?;

        // Re-register
        let server = config.to_managed_server();
        {
            let mut instances = self
                .instances
                .write()
                .map_err(|e| Error::other(format!("Registry lock: {e}")))?;
            instances.insert(id.to_string(), server);
        }
        self.save_configs()?;

        log::info!("Instance '{}' restored from archive", id);
        Ok(())
    }

    /// Get the archive root path.
    pub fn archive_root(&self) -> &Path {
        &self.archive_root
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

    /// Gracefully stop all running server instances.
    pub async fn stop_all(&self) -> usize {
        let ids: Vec<String> = {
            let instances = self.instances.read().map_err(|e| Error::other(e.to_string())).ok();
            instances.map(|m| m.keys().cloned().collect()).unwrap_or_default()
        };
        let mut count = 0;
        for id in &ids {
            if let Ok(server) = self.get_server(id) {
                if server.handle().is_running() {
                    if server.stop().await.is_ok() {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Force-kill all running server instances.
    pub async fn kill_all(&self) -> usize {
        let ids: Vec<String> = {
            let instances = self.instances.read().map_err(|e| Error::other(e.to_string())).ok();
            instances.map(|m| m.keys().cloned().collect()).unwrap_or_default()
        };
        let mut count = 0;
        for id in &ids {
            if let Ok(server) = self.get_server(id) {
                if server.handle().is_running() {
                    let _ = server.kill().await;
                    count += 1;
                }
            }
        }
        count
    }

    /// Send a console command to a running server.
    pub async fn send_command(&self, id: &str, cmd: &str) -> Result<(), Error> {
        let server = self.get_server(id)?;
        server.send_command(cmd).await
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

/// Generate the JSON Schema for [`InstanceConfig`].
pub fn instance_config_schema() -> serde_json::Value {
    let schema = schemars::schema_for!(InstanceConfig);
    serde_json::to_value(&schema).unwrap_or_default()
}

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
