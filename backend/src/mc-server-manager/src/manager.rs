//! High-level managed server instance.
//!
//! [`ManagedServer`] wraps a [`ServerInstance`] with automatic log capture,
//! player tracking, and server.properties management — all exposed through
//! a thread-safe [`ServerHandle`].

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

use crate::error::Error;
use crate::instance::{ServerConfig, ServerInstance};
use crate::log::LogManager;
use crate::player::PlayerTracker;
use crate::properties::ServerProperties;

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
        }
    }

    // ── Lifecycle ──────────────────────────────────────────────────

    /// Start the server process.
    ///
    /// A background task reads stdout and feeds the log manager and
    /// player tracker automatically.
    pub async fn start(&self) -> Result<(), Error> {
        if self.handle.is_running() {
            return Err(Error::other("Server is already running"));
        }

        // Ensure data directory exists
        tokio::fs::create_dir_all(&self.config.server_dir).await?;

        let instance = ServerInstance::start(&self.config).await?;
        let shared = self.handle.clone();

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
    pub fn send_command(&self, cmd: &str) -> Result<(), Error> {
        let proc = self.process.blocking_lock();
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
}
