//! Sub-modules for [`ManagedServer`] — process lifecycle, world operations,
//! and mod/plugin management.

pub mod mods;
pub mod server;
pub mod world;

pub use mods::{ModInfo, ModpackInfo};
pub use server::{ManagedServer, ServerHandle, ServerStatus};
