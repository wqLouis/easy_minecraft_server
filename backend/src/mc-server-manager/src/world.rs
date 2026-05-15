//! World management for Minecraft server instances.
//!
//! Provides utilities for detecting, inspecting, backing up, and restoring
//! Minecraft world directories.

use std::io::{self, Write};
use std::path::Path;

use serde::Serialize;

use crate::error::Error;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Information about a Minecraft world directory.
#[derive(Debug, Clone, Serialize)]
pub struct WorldInfo {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub size_human: String,
    pub last_modified: String,
    pub region_files: usize,
    pub player_data_files: usize,
}

/// A backup file entry.
#[derive(Debug, Clone, Serialize)]
pub struct BackupEntry {
    pub filename: String,
    pub path: String,
    pub size_bytes: u64,
    pub size_human: String,
    pub created_at: String,
    pub worlds_included: Vec<String>,
}

/// A command history entry.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    pub command: String,
    pub sent_at: String,
}

// ---------------------------------------------------------------------------
// World detection
// ---------------------------------------------------------------------------

/// Check whether a directory is a valid Minecraft world.
///
/// A Minecraft world is identified by the presence of either:
/// - a `level.dat` file (the world data file)
/// - a `region/` subdirectory (containing `.mca` region files)
pub fn is_minecraft_world(path: &Path) -> bool {
    path.join("level.dat").exists() || path.join("region").is_dir()
}

/// Recursively compute the total size (in bytes) of all files under `path`.
pub fn dir_size(path: &Path) -> io::Result<u64> {
    fn walk(dir: &Path, total: &mut u64) -> io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, total)?;
            } else {
                *total += entry.metadata()?.len();
            }
        }
        Ok(())
    }
    let mut total = 0;
    walk(path, &mut total)?;
    Ok(total)
}

/// Format a byte count into a human-readable string (e.g. "1.23 MB").
pub fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Count files with a given extension inside a directory.
pub fn count_files_in(dir: &Path, extension: &str) -> io::Result<usize> {
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().map_or(false, |e| e == extension) {
            count += 1;
        }
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// ZIP archive helpers
// ---------------------------------------------------------------------------

/// Recursively add a directory to a ZIP archive.
pub fn add_dir_to_zip<W: Write + io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir_path: &Path,
    prefix: &str,
    options: &zip::write::FileOptions<'_, ()>,
) -> io::Result<()> {
    for entry in std::fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let zip_path = format!("{}/{}", prefix, name);

        if path.is_dir() {
            zip.add_directory(&zip_path, *options)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            add_dir_to_zip(zip, &path, &zip_path, options)?;
        } else {
            let data = std::fs::read(&path)?;
            zip.start_file(&zip_path, *options)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            zip.write_all(&data)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    }
    Ok(())
}

/// Create a ZIP archive in memory containing the given world directories.
///
/// `worlds` is a list of `(name, path on disk)` pairs.
pub fn create_worlds_zip(worlds: &[(&str, &Path)]) -> Result<Vec<u8>, Error> {
    let mut buffer = io::Cursor::new(Vec::new());
    {
        let mut zip_writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        for &(name, dir_path) in worlds {
            if dir_path.is_dir() {
                // Add the directory entry
                zip_writer
                    .add_directory(name, options)
                    .map_err(|e| Error::other(format!("Failed to add dir to zip: {e}")))?;
                add_dir_to_zip(&mut zip_writer, dir_path, name, &options)?;
            }
        }

        zip_writer
            .finish()
            .map_err(|e| Error::other(format!("Failed to finalize zip: {e}")))?;
    }
    Ok(buffer.into_inner())
}

/// Extract a ZIP archive into `target_dir`. Returns the name of the top-level
/// directory (the first entry's root folder).
pub fn extract_world_zip(data: &[u8], target_dir: &Path) -> Result<String, Error> {
    let reader = io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| Error::other(format!("Invalid ZIP: {e}")))?;

    // Determine the top-level directory name from the first entry
    let top_name = (0..archive.len())
        .find_map(|i| {
            let entry = archive.by_index(i).ok()?;
            let name = entry.name().to_string();
            name.split('/').next().map(|s| {
                if s.is_empty() { "world".to_string() } else { s.to_string() }
            })
        })
        .unwrap_or_else(|| "world".to_string());

    archive
        .extract(target_dir)
        .map_err(|e| Error::other(format!("Failed to extract ZIP: {e}")))?;

    Ok(top_name)
}

// ---------------------------------------------------------------------------
// World scanning
// ---------------------------------------------------------------------------

/// Scan `server_dir` for Minecraft world directories and return their info.
pub fn scan_worlds(server_dir: &Path) -> Result<Vec<WorldInfo>, Error> {
    if !server_dir.is_dir() {
        return Err(Error::other(format!(
            "Server directory not found: {}",
            server_dir.display()
        )));
    }

    let mut worlds = Vec::new();
    let mut entries = std::fs::read_dir(server_dir)
        .map_err(|e| Error::other(format!("Failed to read server dir: {e}")))?;

    while let Some(entry) = entries.next().transpose()? {
        let path = entry.path();
        if path.is_dir() && is_minecraft_world(&path) {
            let name = entry.file_name().to_string_lossy().to_string();
            let size = dir_size(&path).unwrap_or(0);
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_default();
            let region_count = count_files_in(&path.join("region"), "mca").unwrap_or(0);
            let playerdata_count = count_files_in(&path.join("playerdata"), "dat").unwrap_or(0);

            worlds.push(WorldInfo {
                name,
                path: path.to_string_lossy().to_string(),
                size_bytes: size,
                size_human: human_size(size),
                last_modified: modified,
                region_files: region_count,
                player_data_files: playerdata_count,
            });
        }
    }

    Ok(worlds)
}
