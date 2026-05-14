//! `mc-server-manager` — Spawn, monitor, and control Minecraft Java Edition
//! server processes.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use mc_server_manager::{ServerConfig, ServerInstance};
//!
//! # async fn example() {
//! let config = ServerConfig::new(
//!     "/srv/minecraft/paper-1.21.jar",
//!     "/usr/bin/java",
//!     "1G",
//!     "4G",
//!     "/srv/minecraft",
//! );
//!
//! let mut server = ServerInstance::start(config).await.unwrap();
//!
//! // Read server stdout
//! while let Some(line) = server.stdout_line().await {
//!     println!("[MC] {line}");
//! }
//! # }
//! ```

use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Java process exited prematurely (status: {0:?})")]
    PrematureExit(std::process::ExitStatus),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to send command to server stdin")]
    StdinSend,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Describes a Minecraft server to launch.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Path to the server JAR file (e.g. `paper-1.21.jar`).
    pub jar_path: PathBuf,
    /// Path to the Java executable.
    pub java_path: PathBuf,
    /// Initial heap size (e.g. `"512M"`, `"1G"`).
    pub min_memory: String,
    /// Maximum heap size (e.g. `"2G"`, `"4G"`).
    pub max_memory: String,
    /// Working directory for the server (logs, worlds, configs are written
    /// relative to this).
    pub server_dir: PathBuf,
    /// Additional JVM flags.
    pub jvm_args: Vec<String>,
}

impl ServerConfig {
    /// Create a new server configuration.
    ///
    /// `jvm_args` defaults to an empty vec. Use [`with_jvm_args`](Self::with_jvm_args)
    /// to customise.
    pub fn new(
        jar_path: impl Into<PathBuf>,
        java_path: impl Into<PathBuf>,
        min_memory: impl Into<String>,
        max_memory: impl Into<String>,
        server_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            jar_path: jar_path.into(),
            java_path: java_path.into(),
            min_memory: min_memory.into(),
            max_memory: max_memory.into(),
            server_dir: server_dir.into(),
            jvm_args: Vec::new(),
        }
    }

    /// Add extra JVM flags.
    pub fn with_jvm_args(mut self, args: Vec<String>) -> Self {
        self.jvm_args = args;
        self
    }
}

// ---------------------------------------------------------------------------
// Server instance
// ---------------------------------------------------------------------------

/// A running Minecraft server instance.
///
/// Drop the instance (or call [`stop`](Self::stop)) to gracefully shut
/// the server down.
pub struct ServerInstance {
    child: Child,
    stdin_tx: mpsc::UnboundedSender<String>,
    stdout_rx: mpsc::Receiver<String>,
}

impl ServerInstance {
    /// Launch the server. Returns once the JVM has started (not when the
    /// server is fully loaded — that may take a while).
    pub async fn start(config: ServerConfig) -> Result<Self, Error> {
        let mut cmd = Command::new(&config.java_path);
        cmd.current_dir(&config.server_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .arg(format!("-Xms{}", config.min_memory))
            .arg(format!("-Xmx{}", config.max_memory))
            .args(&config.jvm_args)
            .arg("-jar")
            .arg(&config.jar_path)
            .arg("nogui");

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "failed to open server stdin",
            ))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "failed to open server stdout",
            ))
        })?;

        // Channel for sending commands to the server's stdin
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();

        // Spawn a task that writes commands to server stdin
        tokio::spawn(async move {
            let mut writer = BufWriter::new(stdin);
            while let Some(cmd) = stdin_rx.recv().await {
                let _ = writer.write_all(cmd.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
                let _ = writer.flush().await;
            }
        });

        // Channel for reading server stdout lines
        let (stdout_tx, stdout_rx) = mpsc::channel(256);

        // Spawn a task that reads server stdout line-by-line
        let mut reader = BufReader::new(stdout);
        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break, // EOF / error
                    Ok(_) => {
                        let trimmed = line.trim_end().to_string();
                        if stdout_tx.send(trimmed).await.is_err() {
                            break; // receiver dropped
                        }
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin_tx,
            stdout_rx,
        })
    }

    // ── Control ────────────────────────────────────────────────────

    /// Send a command to the server console (e.g. `"say Hello"`, `"stop"`).
    pub fn send_command(&self, cmd: impl Into<String>) -> Result<(), Error> {
        self.stdin_tx
            .send(cmd.into())
            .map_err(|_| Error::StdinSend)
    }

    /// Gracefully stop the server by sending the `stop` command, then wait
    /// for the process to exit.
    pub async fn stop(&mut self) -> Result<(), Error> {
        self.send_command("stop")?;
        self.child.wait().await?;
        Ok(())
    }

    /// Force-kill the server process.
    pub async fn kill(&mut self) -> Result<(), Error> {
        self.child.kill().await?;
        self.child.wait().await?;
        Ok(())
    }

    // ── Status ─────────────────────────────────────────────────────

    /// Returns `true` if the server process is still running.
    pub fn is_running(&mut self) -> bool {
        self.child
            .try_wait()
            .ok()
            .map(|s| s.is_none())
            .unwrap_or(false)
    }

    // ── Streaming stdout ───────────────────────────────────────────

    /// Read the next line from server stdout, blocking until one is
    /// available or the stream ends.
    pub async fn stdout_line(&mut self) -> Option<String> {
        self.stdout_rx.recv().await
    }

    /// Get a mutable reference to the stdout receiver for use in `select!`
    /// loops.
    pub fn stdout_rx(&mut self) -> &mut mpsc::Receiver<String> {
        &mut self.stdout_rx
    }
}

// ---------------------------------------------------------------------------
// Graceful shutdown on drop
// ---------------------------------------------------------------------------

impl Drop for ServerInstance {
    fn drop(&mut self) {
        if self.is_running() {
            let _ = self.stdin_tx.send("stop".to_string());
        }
    }
}
