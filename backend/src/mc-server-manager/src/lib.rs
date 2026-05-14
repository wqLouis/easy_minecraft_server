//! `mc-server-manager` — Spawn, monitor, and control Minecraft Java Edition
//! server processes.
//!
//! This crate provides both low-level process control ([`ServerInstance`])
//! and a high-level managed server ([`ManagedServer`]) with automatic log
//! capture, player tracking, and server.properties management.
//!
//! The [`mc-server-installer`] subcrate is re-exported for convenience so
//! you can download server JARs with the same dependency.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use mc_server_manager::{ManagedServer, ServerConfig};
//!
//! # async fn example() {
//! let config = ServerConfig::new(
//!     "/srv/minecraft/server.jar",
//!     "/usr/bin/java",
//!     "1G",        // min memory
//!     "4G",        // max memory
//!     "/srv/minecraft",
//! );
//!
//! // Create a managed server
//! let mut server = ManagedServer::new(
//!     "main".into(),
//!     "Main Server".into(),
//!     config,
//!     "/srv/minecraft/data".into(),
//! );
//!
//! // Start it
//! server.start().await.unwrap();
//!
//! // Send console commands
//! server.send_command("say Server is live!").unwrap();
//!
//! // Get a thread-safe handle for monitoring
//! let handle = server.handle();
//!
//! // Check status via the handle
//! let status = handle.status().await;
//! println!("{:?}", status);
//!
//! // Read recent logs
//! for line in handle.logs_tail(10).await {
//!     println!("[LOG] {line}");
//! }
//!
//! // Subscribe to live logs
//! let mut rx = handle.subscribe_logs();
//!
//! // Stop gracefully
//! server.stop().await.unwrap();
//! # }
//! ```

// Re-export the installer subcrate
pub use mc_server_installer;

// ---------------------------------------------------------------------------
// Public modules
// ---------------------------------------------------------------------------

mod error;
pub mod instance;
pub mod log;
pub mod manager;
pub mod player;
pub mod properties;
pub mod registry;
pub mod version;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use error::Error;
pub use instance::{ServerConfig, ServerInstance};
pub use log::LogManager;
pub use manager::{ManagedServer, ServerHandle, ServerStatus};
pub use player::{PlayerInfo, PlayerTracker};
pub use properties::ServerProperties;
pub use registry::{InstanceConfig, InstanceSummary, ServerRegistry};
pub use version::{fetch_latest, fetch_versions, list_providers, parse_provider, ProviderInfo};
