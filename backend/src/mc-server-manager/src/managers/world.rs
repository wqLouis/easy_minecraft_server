//! World management methods for [`ManagedServer`] — backup, restore,
//! list worlds, manage backup files.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::world::{self as wm, BackupEntry, WorldInfo};

use super::server::ManagedServer;

impl ManagedServer {
    // ── World listing ─────────────────────────────────────────────

    /// Scan the server directory and return info about all Minecraft
    /// worlds found.
    pub fn list_worlds(&self) -> Result<Vec<WorldInfo>, Error> {
        wm::scan_worlds(&self.config.server_dir)
    }

    // ── Backup / restore ──────────────────────────────────────────

    /// Create a ZIP backup of the given world directories and write it
    /// to `backup_path`.
    pub fn backup_worlds(&self, names: &[String], backup_path: &Path) -> Result<(), Error> {
        let worlds: Vec<(String, PathBuf)> = names
            .iter()
            .map(|n| (n.clone(), self.config.server_dir.join(n)))
            .collect();
        let world_refs: Vec<(&str, &Path)> = worlds
            .iter()
            .map(|(n, p)| (n.as_str(), p.as_path()))
            .collect();
        let data = wm::create_worlds_zip(&world_refs)?;
        if let Some(p) = backup_path.parent() {
            std::fs::create_dir_all(p).map_err(|e| Error::other(e.to_string()))?;
        }
        std::fs::write(backup_path, &data).map_err(|e| Error::other(e.to_string()))?;
        Ok(())
    }

    /// Extract a world ZIP archive into the server directory. Returns
    /// the extracted top-level folder name.
    pub fn extract_world_zip(&self, data: &[u8]) -> Result<String, Error> {
        wm::extract_world_zip(data, &self.config.server_dir)
    }

    /// Delete a world directory from the server folder.
    pub fn delete_world_dir(&self, name: &str) -> Result<(), Error> {
        let p = self.config.server_dir.join(name);
        if !p.is_dir() {
            return Err(Error::other(format!("World '{name}' not found")));
        }
        if !wm::is_minecraft_world(&p) {
            return Err(Error::other(format!("'{name}' not a valid world")));
        }
        std::fs::remove_dir_all(&p).map_err(|e| Error::other(e.to_string()))?;
        Ok(())
    }

    // ── Backups listing ───────────────────────────────────────────

    /// Return the path where backups for this server are stored.
    pub fn backups_dir(&self) -> PathBuf {
        let p = PathBuf::from("./data/backups").join(&self.handle.id);
        let _ = std::fs::create_dir_all(&p);
        p
    }

    /// List all backup ZIP files for this server.
    pub fn list_backups(&self) -> Result<Vec<BackupEntry>, Error> {
        let dir = self.backups_dir();
        let mut b = vec![];
        if dir.is_dir() {
            for e in std::fs::read_dir(&dir).map_err(|e| Error::other(e.to_string()))? {
                let e = e.map_err(|e| Error::other(e.to_string()))?;
                if e.path().extension().map_or(false, |x| x == "zip") {
                    let meta = e.metadata().ok();
                    b.push(BackupEntry {
                        filename: e.file_name().to_string_lossy().to_string(),
                        path: e.path().to_string_lossy().to_string(),
                        size_bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                        size_human: wm::human_size(
                            meta.as_ref().map(|m| m.len()).unwrap_or(0),
                        ),
                        created_at: meta
                            .and_then(|m| m.modified().ok())
                            .map(|t| {
                                let dt: chrono::DateTime<chrono::Utc> = t.into();
                                dt.to_rfc3339()
                            })
                            .unwrap_or_default(),
                        worlds_included: vec![],
                    });
                }
            }
        }
        b.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(b)
    }
}
