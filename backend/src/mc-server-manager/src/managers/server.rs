//! Core server process management — [`ManagedServer`], [`ServerHandle`],
//! [`ServerStatus`], process lifecycle, and command history.

use crate::error::Error;
use crate::instance::{ServerConfig, ServerInstance};
use crate::log::LogManager;
use crate::player::PlayerTracker;
use crate::properties::ServerProperties;
use crate::version::parse_provider;
use crate::world::HistoryEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, broadcast};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Snapshot of a server's current status.
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

/// A thread-safe handle to a running (or stopped) managed server.
///
/// Cloning the handle is cheap — all state is behind `Arc`.
#[derive(Clone)]
pub struct ServerHandle {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) log_manager: Arc<Mutex<LogManager>>,
    pub(crate) player_tracker: Arc<Mutex<PlayerTracker>>,
    pub(crate) properties: Arc<std::sync::RwLock<ServerProperties>>,
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
    pub async fn logs_tail(&self, n: usize) -> Vec<String> {
        self.log_manager.lock().await.tail(n)
    }
    pub async fn logs_all(&self) -> Vec<String> {
        self.log_manager.lock().await.all()
    }
    pub fn subscribe_logs(&self) -> broadcast::Receiver<String> {
        self.log_tx.subscribe()
    }
    pub async fn online_players(&self) -> Vec<String> {
        self.player_tracker.lock().await.online_players()
    }
    pub async fn player_count(&self) -> usize {
        self.player_tracker.lock().await.player_count()
    }
    pub fn property(&self, key: &str) -> Option<String> {
        let path = self.properties.read().ok()?.path().to_path_buf();
        ServerProperties::load(path)
            .ok()
            .and_then(|p| p.get(key).map(|s| s.to_string()))
    }
    pub fn set_property(&self, key: String, value: String) {
        if let Ok(mut p) = self.properties.write() {
            p.set(key, value);
        }
    }
    pub fn save_properties(&self) -> Result<(), Error> {
        let p = self.properties.read().map_err(|_| Error::other("lock"))?;
        p.save()
    }
    pub fn all_properties(&self) -> HashMap<String, String> {
        let path = self
            .properties
            .read()
            .map(|p| p.path().to_path_buf())
            .unwrap_or_default();
        ServerProperties::load(path)
            .map(|p| p.all().clone())
            .unwrap_or_default()
    }
    pub fn properties_path(&self) -> PathBuf {
        self.properties
            .read()
            .map(|p| p.path().to_path_buf())
            .unwrap_or_default()
    }
    pub fn update_properties(&self, changes: HashMap<String, String>) -> Result<bool, Error> {
        {
            let mut p = self.properties.write().map_err(|_| Error::other("lock"))?;
            for (k, v) in &changes {
                p.set(k.clone(), v.clone());
            }
            p.save()?;
        }
        Ok(self.is_running())
    }
    pub async fn status(&self) -> ServerStatus {
        let players = self.online_players().await;
        let log_lines = self.log_manager.lock().await.len();
        let props_count = self.properties.read().map(|p| p.len()).unwrap_or(0);
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

/// A fully managed Minecraft server process with lifecycle control,
/// mods, worlds, properties, and command history.
#[derive(Clone)]
pub struct ManagedServer {
    pub(crate) handle: ServerHandle,
    pub(crate) process: Arc<Mutex<Option<ServerInstance>>>,
    pub(crate) config: ServerConfig,
    pub(crate) provider: String,
    pub(crate) version: String,
}

impl ManagedServer {
    // ── Construction ──────────────────────────────────────────────

    /// Create a new managed server. The server is not started until you
    /// call [`start`](Self::start).
    pub fn new(
        id: String,
        name: String,
        config: ServerConfig,
        _data_dir: PathBuf,
        provider: String,
        version: String,
    ) -> Self {
        let (log_tx, _) = broadcast::channel(1024);
        let pp = config.server_dir.join("server.properties");
        let props = ServerProperties::load(pp).unwrap_or_else(|_| {
            ServerProperties::load(PathBuf::new())
                .unwrap_or_else(|_| ServerProperties::load(PathBuf::from("/dev/null")).unwrap())
        });
        Self {
            handle: ServerHandle {
                id,
                name,
                running: Arc::new(AtomicBool::new(false)),
                log_manager: Arc::new(Mutex::new(LogManager::default())),
                player_tracker: Arc::new(Mutex::new(PlayerTracker::new())),
                properties: Arc::new(std::sync::RwLock::new(props)),
                log_tx,
            },
            process: Arc::new(Mutex::new(None)),
            config,
            provider,
            version,
        }
    }

    // ── Lifecycle ────────────────────────────────────────────────

    /// Start the server. Downloads the JAR if missing, runs installers
    /// for Fabric/Forge/NeoForge, then spawns the JVM process.
    pub async fn start(&mut self) -> Result<(), Error> {
        if self.handle.is_running() {
            return Err(Error::other("Already running"));
        }
        tokio::fs::create_dir_all(&self.config.server_dir).await?;
        let _ = tokio::fs::write(self.config.server_dir.join("eula.txt"), b"eula=true\n").await;
        if !tokio::fs::try_exists(&self.config.jar_path)
            .await
            .unwrap_or(false)
        {
            let info =
                mc_server_installer::fetch_latest(parse_provider(&self.provider)?, &self.version)
                    .await
                    .map_err(|e| Error::other(format!("Download: {e}")))?;
            let is_installer = matches!(
                self.provider.to_lowercase().as_str(),
                "fabric" | "forge" | "neoforge"
            );
            if is_installer {
                let installer = self.config.server_dir.join(".installer.jar");
                log::info!(
                    "Downloading {} {} installer...",
                    self.provider,
                    self.version
                );
                mc_server_installer::download(&info.download_url, &installer)
                    .await
                    .map_err(|e| Error::other(format!("Download installer: {e}")))?;
                log::info!("Running installer...");
                let abs = std::fs::canonicalize(&installer).unwrap_or_else(|_| installer.clone());
                let args: Vec<&str> = match self.provider.to_lowercase().as_str() {
                    "fabric" => vec![
                        "server",
                        "-dir",
                        ".",
                        "-mcversion",
                        &self.version,
                        "-downloadMinecraft",
                    ],
                    _ => vec!["--installServer"],
                };
                let status = tokio::process::Command::new(&self.config.java_path)
                    .arg("-jar")
                    .arg(&abs)
                    .args(&args)
                    .current_dir(&self.config.server_dir)
                    .status()
                    .await
                    .map_err(|e| Error::other(format!("Run installer: {e}")))?;
                if !status.success() {
                    return Err(Error::other(format!("{} installer failed", self.provider)));
                }
                let actual: PathBuf = match self.provider.to_lowercase().as_str() {
                    "fabric" => self.config.server_dir.join("fabric-server-launch.jar"),
                    _ => std::fs::read_dir(&self.config.server_dir)
                        .ok()
                        .into_iter()
                        .flatten()
                        .flatten()
                        .find_map(|e| {
                            let n = e.file_name().to_string_lossy().to_string();
                            if n.ends_with("-server.jar") || n.ends_with("-universal.jar") {
                                Some(e.path())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| self.config.server_dir.join("server.jar")),
                };
                let _ = tokio::fs::remove_file(&installer).await;
                self.config.jar_path = actual;
            } else {
                log::info!("Downloading {} {}...", self.provider, self.version);
                mc_server_installer::download(&info.download_url, &self.config.jar_path)
                    .await
                    .map_err(|e| Error::other(format!("Download: {e}")))?;
            }
        }
        let mut instance = ServerInstance::start(&self.config).await?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if !instance.is_running() {
            return Err(Error::other("Server exited immediately"));
        }
        let mut ift = Some(instance);
        let rx = ift.as_mut().unwrap().take_stdout_rx().expect("stdout_rx");
        self.handle.running.store(true, Ordering::SeqCst);
        {
            let mut p = self.process.lock().await;
            *p = ift.take();
        }
        let sc = self.handle.clone();
        tokio::spawn(async move {
            Self::reader_task(sc, rx).await;
        });
        log::info!("Server '{}' started", self.handle.name());
        Ok(())
    }

    /// Background task that reads server stdout lines, pushes them into
    /// the log manager, updates the player tracker, and broadcasts to
    /// log subscribers.
    async fn reader_task(
        shared: ServerHandle,
        mut rx: tokio::sync::mpsc::Receiver<String>,
    ) {
        while let Some(line) = rx.recv().await {
            shared.log_manager.lock().await.push(line.clone());
            shared.player_tracker.lock().await.process_log_line(&line);
            let _ = shared.log_tx.send(line);
        }
        shared.running.store(false, Ordering::SeqCst);
    }

    /// Gracefully stop the server (sends `stop` command).
    pub async fn stop(&self) -> Result<(), Error> {
        let mut p = self.process.lock().await;
        if let Some(ref mut i) = *p {
            i.stop().await?;
        }
        self.handle.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Force-kill the server process.
    pub async fn kill(&self) -> Result<(), Error> {
        let mut p = self.process.lock().await;
        if let Some(ref mut i) = *p {
            i.kill().await?;
        }
        self.handle.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Send a console command to the running server.
    pub async fn send_command(&self, cmd: &str) -> Result<(), Error> {
        let p = self.process.lock().await;
        p.as_ref()
            .ok_or_else(|| Error::other("Not running"))?
            .send_command(cmd)
    }

    // ── Accessors ────────────────────────────────────────────────

    /// Get a thread-safe handle for monitoring this server.
    pub fn handle(&self) -> &ServerHandle {
        &self.handle
    }

    /// Get the server's launch configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Get the provider name (e.g. "paper", "vanilla").
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Get the Minecraft version string.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get the server's working directory.
    pub fn server_dir(&self) -> &Path {
        &self.config.server_dir
    }

    // ── Command history ──────────────────────────────────────────

    fn cmd_history_path(&self) -> PathBuf {
        PathBuf::from("./data/command_history").join(format!("{}.json", self.handle.id))
    }

    /// Return the recorded command history for this server.
    pub fn command_history(&self) -> Vec<HistoryEntry> {
        std::fs::read_to_string(self.cmd_history_path())
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    }

    /// Record a command in the server's command history (keeps last 500).
    pub fn record_command(&self, command: &str) {
        let mut h = self.command_history();
        h.push(HistoryEntry {
            command: command.to_string(),
            sent_at: chrono::Utc::now().to_rfc3339(),
        });
        if h.len() > 500 {
            h = h.split_off(h.len() - 500);
        }
        let p = self.cmd_history_path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(&p, serde_json::to_string_pretty(&h).unwrap());
    }

    /// Detect the actual loader version from the server's installed files.
    ///
    /// Returns `(dependency_key, version_string)` or `None` if unknown.
    /// This is used by modpack generation to avoid wildcard/range strings
    /// (like `"*"` or `">=0.0.0"`) that contain characters illegal in
    /// Windows paths — some launchers (PCL) use the version string
    /// literally when downloading the loader.
    pub fn detect_loader_version(&self) -> Option<(String, String)> {
        let sd = &self.config.server_dir;
        match self.provider.to_lowercase().as_str() {
            "fabric" => {
                let key = "fabric-loader".to_string();
                // Check libraries/net/fabricmc/fabric-loader/<version>/
                let lib_dir = sd.join("libraries/net/fabricmc/fabric-loader");
                if let Some(v) = Self::find_version_dir(&lib_dir) {
                    return Some((key, v));
                }
                None
            }
            "forge" => {
                let key = "forge".to_string();
                // Forge dirs look like "1.20.1-47.1.0" — extract the part after the dash
                let lib_dir = sd.join("libraries/net/minecraftforge/forge");
                if lib_dir.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(&lib_dir) {
                        // Find the directory with the most recent version
                        let mut best: Option<String> = None;
                        for e in entries.flatten() {
                            if !e.path().is_dir() { continue; }
                            let n = e.file_name().to_string_lossy().to_string();
                            if let Some(ver) = n.split('-').nth(1) {
                                if !ver.is_empty() {
                                    match (&best, ver) {
                                        (None, v) => best = Some(v.to_string()),
                                        (Some(b), v) if v > b.as_str() => best = Some(v.to_string()),
                                        _ => {}
                                    }
                                }
                            }
                        }
                        if let Some(v) = best {
                            return Some((key, v));
                        }
                    }
                }
                None
            }
            "neoforge" => {
                let key = "neoforge".to_string();
                let lib_dir = sd.join("libraries/net/neoforged/neoforge");
                if let Some(v) = Self::find_version_dir(&lib_dir) {
                    return Some((key, v));
                }
                None
            }
            "quilt" => {
                let key = "quilt-loader".to_string();
                let lib_dir = sd.join("libraries/org/quiltmc/quilt-loader");
                if let Some(v) = Self::find_version_dir(&lib_dir) {
                    return Some((key, v));
                }
                None
            }
            _ => None, // paper, purpur, vanilla — no separate loader
        }
    }

    /// Scan a library directory for version-number subdirectories
    /// (e.g. "libraries/net/fabricmc/fabric-loader/0.19.2/") and
    /// return the latest version found.
    fn find_version_dir(dir: &Path) -> Option<String> {
        if !dir.is_dir() {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        let mut best: Option<String> = None;
        for e in entries.flatten() {
            if !e.path().is_dir() { continue; }
            let n = e.file_name().to_string_lossy().to_string();
            // Version directories look like "0.19.2" — numeric with dots only
            if !n.chars().all(|c| c.is_ascii_digit() || c == '.') {
                continue;
            }
            // Skip "." and ".." just in case
            if n == "." || n == ".." { continue; }
            match &best {
                None => best = Some(n),
                Some(b) => {
                    // Compare version strings: if n > b (later version), use n
                    if n > *b { best = Some(n); }
                }
            }
        }
        best
    }
}
