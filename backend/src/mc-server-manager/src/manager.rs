//! High-level managed server instance.
//!
//! [`ManagedServer`] wraps a [`ServerInstance`] with automatic log capture,
//! player tracking, and server.properties management — all exposed through
//! a thread-safe [`ServerHandle`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

use crate::error::Error;
use crate::instance::{ServerConfig, ServerInstance};
use crate::version::parse_provider;
use crate::log::LogManager;
use crate::player::PlayerTracker;
use crate::properties::ServerProperties;
use crate::world::{self as world_mod, WorldInfo, BackupEntry, HistoryEntry};

// ---------------------------------------------------------------------------
// Server status
// ---------------------------------------------------------------------------

/// Snapshot of the server's current state.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStatus {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub online_players: Vec<String>,
    pub player_count: usize,
    pub log_lines: usize,
    pub properties_count: usize,
}

// ---------------------------------------------------------------------------
// ServerHandle (thread-safe shared state)
// ---------------------------------------------------------------------------

/// Thread-safe handle to shared server state.
///
/// Clone this handle to pass it between tasks. All state access is
/// done through `Arc` + `Mutex` so the background log reader task and
/// the public API can operate concurrently.
#[derive(Clone)]
pub struct ServerHandle {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) log_manager: Arc<Mutex<LogManager>>,
    pub(crate) player_tracker: Arc<Mutex<PlayerTracker>>,
    pub(crate) properties: Arc<std::sync::RwLock<ServerProperties>>,
    /// Broadcast sender for real-time log streaming.
    pub(crate) log_tx: broadcast::Sender<String>,
}

impl ServerHandle {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Return the last `n` log lines.
    pub async fn logs_tail(&self, n: usize) -> Vec<String> {
        self.log_manager.lock().await.tail(n)
    }

    /// Return all buffered log lines.
    pub async fn logs_all(&self) -> Vec<String> {
        self.log_manager.lock().await.all()
    }

    /// Subscribe to real-time log lines.
    pub fn subscribe_logs(&self) -> broadcast::Receiver<String> {
        self.log_tx.subscribe()
    }

    /// Return the list of online player names.
    pub async fn online_players(&self) -> Vec<String> {
        self.player_tracker.lock().await.online_players()
    }

    /// Return the number of online players.
    pub async fn player_count(&self) -> usize {
        self.player_tracker.lock().await.player_count()
    }

    /// Read a server property by key.
    pub fn property(&self, key: &str) -> Option<String> {
        let p = self.properties.read().ok()?;
        p.get(key).map(|s| s.to_string())
    }

    /// Set a server property (not persisted until [`save_properties`](Self::save_properties)).
    pub fn set_property(&self, key: String, value: String) {
        if let Ok(mut p) = self.properties.write() {
            p.set(key, value);
        }
    }

    /// Persist the current properties to disk.
    pub fn save_properties(&self) -> Result<(), Error> {
        let p = self
            .properties
            .read()
            .map_err(|_| Error::other("properties lock poisoned"))?;
        p.save()
    }

    /// Return all properties as a HashMap.
    pub fn all_properties(&self) -> HashMap<String, String> {
        self.properties
            .read()
            .map(|p| p.all().clone())
            .unwrap_or_default()
    }

    /// Return the path to the server.properties file.
    pub fn properties_path(&self) -> PathBuf {
        self.properties
            .read()
            .map(|p| p.path().to_path_buf())
            .unwrap_or_default()
    }

    /// Update properties from a map of changes (partial merge), persist to disk.
    /// Returns `true` if the server is running (requiring a restart for changes to apply).
    pub fn update_properties(&self, changes: HashMap<String, String>) -> Result<bool, Error> {
        {
            let mut p = self
                .properties
                .write()
                .map_err(|_| Error::other("properties lock poisoned"))?;
            for (key, value) in &changes {
                p.set(key.clone(), value.clone());
            }
            p.save()?;
        }
        Ok(self.is_running())
    }

    /// Take a snapshot of the current server status.
    pub async fn status(&self) -> ServerStatus {
        let players = self.online_players().await;
        let log_lines = self.log_manager.lock().await.len();
        let props_count = self
            .properties
            .read()
            .map(|p| p.len())
            .unwrap_or(0);

        ServerStatus {
            id: self.id.clone(),
            name: self.name.clone(),
            running: self.is_running(),
            online_players: players.clone(),
            player_count: players.len(),
            log_lines,
            properties_count: props_count,
        }
    }
}

// ---------------------------------------------------------------------------
// ManagedServer
// ---------------------------------------------------------------------------

/// A fully managed Minecraft server instance.
///
/// # Features
///
/// | Feature | How |
/// |---------|-----|
/// | Start / Stop | Spawns JVM process; sends `stop` on shutdown |
/// | Console commands | Sends arbitrary strings via stdin |
/// | Log capture | Ring buffer + broadcast streaming |
/// | Player tracking | Parses join/leave log lines |
/// | Server properties | Read/write `server.properties` |
///
/// # Example
///
/// ```rust,no_run
/// use mc_server_manager::{ManagedServer, ServerConfig};
///
/// # async fn example() {
/// let config = ServerConfig::new(
///     "/srv/minecraft/server.jar", "/usr/bin/java",
///     "1G", "4G", "/srv/minecraft",
/// );
///
/// let mut server = ManagedServer::new(
///     "main".into(), "Main Server".into(),
///     config, "/srv/minecraft/data".into(),
///     "paper".into(), "1.21.4".into(),
/// );
///
/// server.start().await.unwrap();
/// server.send_command("say Hello!").unwrap();
/// let handle = server.handle();
/// println!("{:?}", handle.status().await);
/// server.stop().await.unwrap();
/// # }
/// ```
#[derive(Clone)]
pub struct ManagedServer {
    handle: ServerHandle,
    /// The underlying low-level server instance (moved into the reader
    /// task on start, so only present before start or after stop).
    process: Arc<Mutex<Option<ServerInstance>>>,
    config: ServerConfig,
    provider: String,
    version: String,
}

impl ManagedServer {
    /// Create a new managed server.
    ///
    /// The server is **not** started — call [`start`](Self::start).
    pub fn new(
        id: String,
        name: String,
        config: ServerConfig,
        _data_dir: PathBuf,
        provider: String,
        version: String,
    ) -> Self {
        let (log_tx, _) = broadcast::channel(1024);

        // Try to load server.properties from the server directory
        let properties_path = config.server_dir.join("server.properties");
        let properties = ServerProperties::load(properties_path).unwrap_or_else(|e| {
            log::warn!("Failed to load server.properties: {e}");
            ServerProperties::load(PathBuf::new()).unwrap_or_else(|_| {
                ServerProperties::load(PathBuf::from("/dev/null")).unwrap()
            })
        });

        Self {
            handle: ServerHandle {
                id,
                name,
                running: Arc::new(AtomicBool::new(false)),
                log_manager: Arc::new(Mutex::new(LogManager::default())),
                player_tracker: Arc::new(Mutex::new(PlayerTracker::new())),
                properties: Arc::new(std::sync::RwLock::new(properties)),
                log_tx,
            },
            process: Arc::new(Mutex::new(None)),
            config,
            provider,
            version,
        }
    }

    // ── Lifecycle ──────────────────────────────────────────────────

    /// Start the server process.
    ///
    /// If the server JAR does not exist locally, it is downloaded
    /// automatically from the provider's API.
    /// For installer-based providers (Fabric, Forge, NeoForge), the
    /// installer is run to produce the actual server JAR, and
    /// `self.config.jar_path` is updated to point to it.
    pub async fn start(&mut self) -> Result<(), Error> {
        if self.handle.is_running() {
            return Err(Error::other("Server is already running"));
        }

        // Ensure data directory exists
        tokio::fs::create_dir_all(&self.config.server_dir).await?;

        // Auto-accept EULA so the server doesn't exit on first start
        let eula_path = self.config.server_dir.join("eula.txt");
        let _ = tokio::fs::write(&eula_path, b"eula=true\n").await;

        // Auto-download JAR if it doesn't exist
        let jar_path = &self.config.jar_path;
        if !tokio::fs::try_exists(jar_path).await.unwrap_or(false) {
            let info = mc_server_installer::fetch_latest(
                parse_provider(&self.provider)?,
                &self.version,
            )
            .await
            .map_err(|e| Error::other(format!("Failed to resolve download URL: {e}")))?;

            let is_installer = matches!(self.provider.to_lowercase().as_str(), "fabric" | "forge" | "neoforge");

            if is_installer {
                // Download installer to a temp path, run it, then point jar_path to the result
                let installer_jar = self.config.server_dir.join(".installer.jar");
                log::info!("Downloading {} {} installer to {}...", self.provider, self.version, installer_jar.display());
                mc_server_installer::download(&info.download_url, &installer_jar)
                    .await
                    .map_err(|e| Error::other(format!("Failed to download installer: {e}")))?;

                log::info!("Running installer for {} {}...", self.provider, self.version);
                // Resolve installer path to absolute — JVM's CWD differs from backend's
                let abs_installer = std::fs::canonicalize(&installer_jar)
                    .unwrap_or_else(|_| installer_jar.clone());
                let status = tokio::process::Command::new(&self.config.java_path)
                    .arg("-jar")
                    .arg(&abs_installer)
                    .args(match self.provider.to_lowercase().as_str() {
                        "fabric" => vec![
                            "server",
                            "-dir",
                            ".",   // CWD is already server_dir
                            "-mcversion",
                            &self.version,
                            "-downloadMinecraft",
                        ],
                        _ => vec!["--installServer"],
                    })
                    .current_dir(&self.config.server_dir)
                    .status()
                    .await
                    .map_err(|e| Error::other(format!("Failed to run installer: {e}")))?;

                if !status.success() {
                    return Err(Error::other(format!(
                        "{} installer exited with error",
                        self.provider
                    )));
                }

                // Determine the actual server JAR produced by the installer
                let actual_jar = match self.provider.to_lowercase().as_str() {
                    "fabric" => self.config.server_dir.join("fabric-server-launch.jar"),
                    _ => {
                        // Forge/NeoForge: look for forge-{version}-server.jar / -universal.jar
                        let mut found: Option<std::path::PathBuf> = None;
                        if let Ok(entries) = std::fs::read_dir(&self.config.server_dir) {
                            for entry in entries.flatten() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                if name.ends_with("-server.jar") || name.ends_with("-universal.jar") {
                                    found = Some(entry.path());
                                    break;
                                }
                            }
                        }
                        found.unwrap_or_else(|| self.config.server_dir.join("server.jar"))
                    }
                };

                // Clean up installer JAR
                let _ = tokio::fs::remove_file(&installer_jar).await;

                // Point config to the actual server JAR
                self.config.jar_path = actual_jar;
                log::info!(
                    "Installer finished, server JAR: {}",
                    self.config.jar_path.display()
                );
            } else {
                // Direct-download providers (Vanilla, Paper, Purpur, etc.)
                log::info!(
                    "Downloading {} {} to {}...",
                    self.provider, self.version, jar_path.display()
                );
                mc_server_installer::download(&info.download_url, jar_path)
                    .await
                    .map_err(|e| Error::other(format!("Failed to download server JAR: {e}")))?;
                log::info!("Downloaded {} to {}", info.name, jar_path.display());
            }
        }

        let mut instance = ServerInstance::start(&self.config).await?;
        let shared = self.handle.clone();

        // Give the JVM a moment to fail (bad JAR, missing Java, etc.)
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if !instance.is_running() {
            let err = format!(
                "Server '{}' exited immediately — check JAR path and Java installation",
                shared.name()
            );
            log::error!("{}", err);
            return Err(Error::other(err));
        }

        // Take the stdout receiver and spawn the reader task
        let mut instance_for_task = Some(instance);
        let rx = instance_for_task
            .as_mut()
            .unwrap()
            .take_stdout_rx()
            .expect("stdout_rx should be available on fresh instance");

        shared.running.store(true, Ordering::SeqCst);

        // Store the instance (without stdout_rx) for lifecycle control
        {
            let mut proc = self.process.lock().await;
            *proc = instance_for_task.take();
        }

        // Spawn background reader task
        let shared_clone = shared.clone();
        tokio::spawn(async move {
            Self::reader_task(shared_clone, rx).await;
        });

        log::info!(
            "Server '{}' started (JAR: {}, dir: {})",
            shared.name(),
            self.config.jar_path.display(),
            self.config.server_dir.display()
        );

        Ok(())
    }

    /// Background task that reads stdout lines and feeds log + player systems.
    async fn reader_task(shared: ServerHandle, mut rx: tokio::sync::mpsc::Receiver<String>) {
        while let Some(line) = rx.recv().await {
            // Push to log manager
            shared.log_manager.lock().await.push(line.clone());
            // Process for player tracking
            shared.player_tracker.lock().await.process_log_line(&line);
            // Broadcast to subscribers
            let _ = shared.log_tx.send(line);
        }
        // Stream ended — server process exited
        shared.running.store(false, Ordering::SeqCst);
        log::info!("Server '{}' stdout stream ended", shared.name());
    }

    /// Gracefully stop the server (sends `stop` command and waits).
    pub async fn stop(&self) -> Result<(), Error> {
        let mut proc = self.process.lock().await;
        if let Some(ref mut instance) = *proc {
            instance.stop().await?;
        }
        self.handle.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Force-kill the server process.
    pub async fn kill(&self) -> Result<(), Error> {
        let mut proc = self.process.lock().await;
        if let Some(ref mut instance) = *proc {
            instance.kill().await?;
        }
        self.handle.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    // ── Console ────────────────────────────────────────────────────

    /// Send a command to the server console.
    ///
    /// Examples: `"say Hello"`, `"stop"`, `"list"`, `"op Steve"`.
    pub async fn send_command(&self, cmd: &str) -> Result<(), Error> {
        let proc = self.process.lock().await;
        if let Some(ref instance) = *proc {
            instance.send_command(cmd)
        } else {
            Err(Error::other("Server is not running"))
        }
    }

    // ── Accessors ──────────────────────────────────────────────────

    /// Get a thread-safe handle to the server's shared state.
    pub fn handle(&self) -> &ServerHandle {
        &self.handle
    }

    /// Reference to the launch configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    // ── Server directory ───────────────────────────────────────────

    /// Path to the server's working directory.
    pub fn server_dir(&self) -> &Path {
        &self.config.server_dir
    }

    // ── World management ───────────────────────────────────────────

    /// List all Minecraft worlds in the server directory.
    pub fn list_worlds(&self) -> Result<Vec<WorldInfo>, Error> {
        world_mod::scan_worlds(&self.config.server_dir)
    }

    /// Create a backup ZIP for the given worlds.
    pub fn backup_worlds(
        &self,
        world_names: &[String],
        backup_path: &Path,
    ) -> Result<(), Error> {
        let server_dir = &self.config.server_dir;
        let world_paths: Vec<PathBuf> = world_names
            .iter()
            .map(|name| server_dir.join(name))
            .collect();
        let worlds: Vec<(&str, &Path)> = world_names
            .iter()
            .zip(world_paths.iter())
            .map(|(name, path)| (name.as_str(), path.as_path()))
            .collect();

        let zip_data = world_mod::create_worlds_zip(&worlds)?;

        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::other(format!("Failed to create backup dir: {e}")))?;
        }

        std::fs::write(backup_path, &zip_data)
            .map_err(|e| Error::other(format!("Failed to write backup file: {e}")))?;

        Ok(())
    }

    /// Extract a world ZIP into the server directory.
    /// Returns the name of the extracted world.
    pub fn extract_world_zip(&self, data: &[u8]) -> Result<String, Error> {
        world_mod::extract_world_zip(data, &self.config.server_dir)
    }

    /// Delete a world directory (server must be stopped).
    pub fn delete_world_dir(&self, name: &str) -> Result<(), Error> {
        let world_path = self.config.server_dir.join(name);
        if !world_path.is_dir() {
            return Err(Error::other(format!("World '{}' not found", name)));
        }
        if !world_mod::is_minecraft_world(&world_path) {
            return Err(Error::other(format!(
                "'{}' is not a valid Minecraft world",
                name
            )));
        }
        std::fs::remove_dir_all(&world_path)
            .map_err(|e| Error::other(format!("Failed to delete world: {e}")))?;
        log::info!("World '{}' deleted", name);
        Ok(())
    }

    /// Get the backups directory for this instance.
    pub fn backups_dir(&self) -> PathBuf {
        let p = PathBuf::from("./data/backups").join(&self.handle.id);
        let _ = std::fs::create_dir_all(&p);
        p
    }

    /// List all backup files for this instance.
    pub fn list_backups(&self) -> Result<Vec<BackupEntry>, Error> {
        let backup_dir = self.backups_dir();
        let mut backups = Vec::new();

        if backup_dir.is_dir() {
            let mut entries = std::fs::read_dir(&backup_dir)
                .map_err(|e| Error::other(format!("Failed to read backups dir: {e}")))?;
            while let Some(entry) = entries.next().transpose()? {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "zip") {
                    let filename = entry.file_name().to_string_lossy().to_string();
                    let metadata = entry.metadata().ok();
                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                    let modified = metadata
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            let dt: chrono::DateTime<chrono::Utc> = t.into();
                            dt.to_rfc3339()
                        })
                        .unwrap_or_default();

                    backups.push(BackupEntry {
                        filename,
                        path: path.to_string_lossy().to_string(),
                        size_bytes: size,
                        size_human: world_mod::human_size(size),
                        created_at: modified,
                        worlds_included: vec![],
                    });
                }
            }
        }

        // Sort newest first
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(backups)
    }

    // ── Command history ────────────────────────────────────────────

    fn command_history_path(&self) -> PathBuf {
        PathBuf::from(format!("./data/command_history/{}.json", self.handle.id))
    }

    /// Load command history from disk.
    pub fn command_history(&self) -> Vec<HistoryEntry> {
        let path = self.command_history_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Record a command in the history (persisted to disk).
    pub fn record_command(&self, command: &str) {
        let mut history = self.command_history();
        let entry = HistoryEntry {
            command: command.to_string(),
            sent_at: chrono::Utc::now().to_rfc3339(),
        };
        history.push(entry);
        // Keep last 500 commands
        if history.len() > 500 {
            history = history.split_off(history.len() - 500);
        }
        let path = self.command_history_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(&history) {
            let _ = std::fs::write(&path, content);
        }
    }

    // ── Mod / Plugin management ────────────────────────────────────

    /// Return the appropriate mods/plugins directory based on provider type.
    /// - Fabric/Forge/NeoForge → `mods/`
    /// - Everything else → `plugins/`
    pub fn mods_dir(&self) -> PathBuf {
        let dir_name = match self.provider.to_lowercase().as_str() {
            "fabric" | "forge" | "neoforge" => "mods",
            _ => "plugins",
        };
        self.config.server_dir.join(dir_name)
    }

    /// Return the type string: `"mod"` or `"plugin"`.
    pub fn mod_type(&self) -> &str {
        match self.provider.to_lowercase().as_str() {
            "fabric" | "forge" | "neoforge" => "mod",
            _ => "plugin",
        }
    }

    /// List all installed mods/plugins.
    pub fn list_mods(&self) -> Result<Vec<ModInfo>, Error> {
        let dir = self.mods_dir();
        if !dir.is_dir() { return Ok(Vec::new()); }

        let mut items: Vec<ModInfo> = std::fs::read_dir(&dir)
            .map_err(|e| Error::other(format!("Failed to read {}: {e}", dir.display())))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let fname = e.file_name();
                let n = fname.to_string_lossy();
                (n.ends_with(".jar") || n.ends_with(".jar.disabled")) && e.path().is_file()
            })
            .map(|e| {
                let filename = e.file_name().to_string_lossy().to_string();
                let enabled = !filename.ends_with(".disabled");
                let name = filename.strip_suffix(".jar").or_else(|| filename.strip_suffix(".jar.disabled")).unwrap_or(&filename).to_string();
                let meta = e.metadata().ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let modified = meta.and_then(|m| m.modified().ok()).map(|t| { let dt: chrono::DateTime<chrono::Utc> = t.into(); dt.to_rfc3339() }).unwrap_or_default();
                ModInfo { filename, name, enabled, size_bytes: size, size_human: world_mod::human_size(size), last_modified: modified }
            })
            .collect();

        items.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(items)
    }

    /// Install a mod/plugin by downloading from a URL.
    pub async fn install_mod(
        &self,
        download_url: &str,
        filename: &str,
    ) -> Result<ModInfo, Error> {
        let dir = self.mods_dir();
        tokio::fs::create_dir_all(&dir).await
            .map_err(|e| Error::other(format!("Failed to create {}: {e}", dir.display())))?;

        let dest = dir.join(filename);

        mc_server_installer::download(download_url, &dest).await?;

        let metadata = tokio::fs::metadata(&dest).await
            .map_err(|e| Error::other(format!("Failed to read file metadata: {e}")))?;
        let size_bytes = metadata.len();
        let last_modified = metadata.modified().ok()
            .map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.to_rfc3339()
            })
            .unwrap_or_default();

        let name = filename.strip_suffix(".jar").unwrap_or(filename).to_string();

        Ok(ModInfo {
            filename: filename.to_string(),
            name,
            enabled: true,
            size_bytes,
            size_human: world_mod::human_size(size_bytes),
            last_modified,
        })
    }

    /// Delete a mod/plugin file. Server must be stopped.
    pub fn delete_mod(&self, filename: &str) -> Result<(), Error> {
        let dir = self.mods_dir();
        let path = dir.join(filename);
        if !path.exists() {
            // Also try with .disabled extension
            let disabled_path = dir.join(format!("{filename}.disabled"));
            if disabled_path.exists() {
                std::fs::remove_file(&disabled_path)
                    .map_err(|e| Error::other(format!("Failed to delete {filename}: {e}")))?;
                return Ok(());
            }
            return Err(Error::other(format!("Mod/plugin '{filename}' not found")));
        }
        std::fs::remove_file(&path)
            .map_err(|e| Error::other(format!("Failed to delete {filename}: {e}")))?;
        Ok(())
    }

    /// Enable or disable a mod/plugin by renaming `.jar` ↔ `.jar.disabled`.
    pub fn toggle_mod(&self, filename: &str, enabled: bool) -> Result<ModInfo, Error> {
        let dir = self.mods_dir();

        let (src_name, dst_name) = if enabled {
            // Enable: foo.jar.disabled → foo.jar
            let disabled = format!("{filename}.disabled");
            if !dir.join(&disabled).exists() {
                return Err(Error::other(format!(
                    "Disabled file '{disabled}' not found"
                )));
            }
            (disabled, filename.to_string())
        } else {
            // Disable: foo.jar → foo.jar.disabled
            if !dir.join(filename).exists() {
                return Err(Error::other(format!(
                    "File '{filename}' not found"
                )));
            }
            (filename.to_string(), format!("{filename}.disabled"))
        };

        let src = dir.join(&src_name);
        let dst = dir.join(&dst_name);

        std::fs::rename(&src, &dst)
            .map_err(|e| Error::other(format!("Failed to toggle {filename}: {e}")))?;
        let meta = std::fs::metadata(&dst).map_err(|e| Error::other(e.to_string()))?;
        let name = filename.strip_suffix(".jar").unwrap_or(filename).to_string();
        Ok(ModInfo {
            filename: dst_name, name, enabled,
            size_bytes: meta.len(),
            size_human: world_mod::human_size(meta.len()),
            last_modified: meta.modified().ok().map(|t| { let dt: chrono::DateTime<chrono::Utc> = t.into(); dt.to_rfc3339() }).unwrap_or_default(),
        })
    }

    /// Path where generated modpacks are stored.
    pub fn modpack_dir(&self) -> PathBuf {
        PathBuf::from("./data/modpacks").join(&self.handle.id)
    }

    /// Get the path to the most recent generated modpack file, if any.
    pub fn modpack_path(&self) -> Option<PathBuf> {
        let dir = self.modpack_dir();
        if !dir.is_dir() { return None; }
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "mrpack"))
            .map(|e| e.path())
            .collect();
        files.sort_by(|a, b| {
            b.metadata().and_then(|m| m.modified()).ok()
                .cmp(&a.metadata().and_then(|m| m.modified()).ok())
        });
        files.into_iter().next()
    }

    /// Generate a Modrinth modpack (.mrpack) from the installed mods/plugins.
    /// Generate a Modrinth modpack (.mrpack) from the installed mods/plugins.
    ///
    /// Jars are bundled inside `overrides/{mods|plugins}/` for direct extraction,
    /// and also referenced in `modrinth.index.json` with computed SHA1/SHA512 hashes.
    pub fn generate_modpack(
        &self,
        name: &str,
        version: &str,
        include: &[String],
    ) -> Result<ModpackInfo, Error> {
        use std::io::Write;

        let mods = self.list_mods()?;
        let dir = self.mods_dir();
        let selected: Vec<&ModInfo> = if include.is_empty() {
            mods.iter().filter(|m| m.enabled).collect()
        } else {
            mods.iter().filter(|m| include.contains(&m.filename)).collect()
        };
        if selected.is_empty() {
            return Err(Error::other("No mods/plugins selected for modpack"));
        }

        let mod_type = self.mod_type();
        let mods_dir_name = if mod_type == "mod" { "mods" } else { "plugins" };

        // Compute hashes and build files array
        let mut files_entries = Vec::new();
        for m in &selected {
            let jar_path = dir.join(&m.filename);
            let data = std::fs::read(&jar_path)
                .map_err(|e| Error::other(format!("Failed to read {}: {e}", m.filename)))?;

            use sha2::Digest;
            let sha1 = sha1_smol::Sha1::from(&data).digest().to_string();
            let sha512 = format!("{:x}", sha2::Sha512::digest(&data));

            files_entries.push(serde_json::json!({
                "path": format!("{}/{}", mods_dir_name, m.filename),
                "downloads": serde_json::Value::Array(vec![]),
                "hashes": { "sha1": sha1, "sha512": sha512 },
                "fileSize": data.len() as u64,
            }));
        }

        // Build dependencies
        let mut deps = serde_json::json!({ "minecraft": self.version });
        // Add mod-loader dependency key. Note: Fabric uses "fabric-loader", Forge uses "forge", NeoForge uses "neoforge"
        let loader_key = match self.provider.to_lowercase().as_str() {
            "fabric" => Some("fabric-loader"),
            "forge" => Some("forge"),
            "neoforge" => Some("neoforge"),
            "quilt" => Some("quilt-loader"),
            _ => None,
        };
        if let Some(key) = loader_key {
            deps.as_object_mut()
                .map(|obj| obj.insert(key.to_string(), serde_json::Value::String("0.0.0".into())));
        }

        let index_json = serde_json::json!({
            "formatVersion": 1, "game": "minecraft", "versionId": version,
            "name": name,
            "summary": format!("Server modpack generated from {} {}", self.provider, self.version),
            "files": files_entries,
            "dependencies": deps,
        });

        let out_dir = self.modpack_dir();
        std::fs::create_dir_all(&out_dir)
            .map_err(|e| Error::other(format!("Failed to create modpack dir: {e}")))?;
        let safe_name = name.replace(' ', "-").to_lowercase();
        let out_path = out_dir.join(format!("{}-{}-{}.mrpack", self.handle.id, safe_name, version));

        // Build ZIP
        let file = std::fs::File::create(&out_path)
            .map_err(|e| Error::other(format!("Failed to create modpack: {e}")))?;
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o644);

        // Write modrinth.index.json at ZIP root
        zip.start_file("modrinth.index.json", opts)
            .map_err(|e| Error::other(format!("Failed to write index: {e}")))?;
        zip.write_all(serde_json::to_string_pretty(&index_json)
            .map_err(|e| Error::other(format!("Failed to serialize index: {e}")))?.as_bytes())
            .map_err(|e| Error::other(format!("Failed to write index: {e}")))?;

        // Bundle jar files under overrides/{mods|plugins}/ for direct extraction
        zip.add_directory(format!("overrides/{mods_dir_name}"), opts)
            .map_err(|e| Error::other(format!("Failed to add dir: {e}")))?;
        for m in &selected {
            let jar_path = dir.join(&m.filename);
            if !jar_path.is_file() { continue; }
            let data = std::fs::read(&jar_path)
                .map_err(|e| Error::other(format!("Failed to read {}: {e}", m.filename)))?;
            zip.start_file(format!("overrides/{}/{}", mods_dir_name, m.filename), opts)
                .map_err(|e| Error::other(format!("Failed to add {}: {e}", m.filename)))?;
            zip.write_all(&data)
                .map_err(|e| Error::other(format!("Failed to write {}: {e}", m.filename)))?;
        }

        zip.finish()
            .map_err(|e| Error::other(format!("Failed to finalize ZIP: {e}")))?;
        let size = std::fs::metadata(&out_path).map_err(|e| Error::other(e.to_string()))?.len();

        Ok(ModpackInfo {
            name: name.to_string(), version: version.to_string(),
            file_path: out_path.to_string_lossy().to_string(),
            size_bytes: size, include_count: selected.len(),
        })
    }
}

// ═══════════════════════════════════════════════════════════════════
// Mod / Plugin types
// ═══════════════════════════════════════════════════════════════════

/// Information about an installed mod or plugin.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModInfo {
    pub filename: String,
    pub name: String,
    pub enabled: bool,
    pub size_bytes: u64,
    pub size_human: String,
    pub last_modified: String,
}

/// Information about a generated modpack.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModpackInfo {
    pub name: String,
    pub version: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub include_count: usize,
}
