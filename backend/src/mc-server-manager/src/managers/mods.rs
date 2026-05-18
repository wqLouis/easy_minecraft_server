//! Mod/plugin management for [`ManagedServer`] — list, install, delete,
//! toggle, and generate Modrinth modpacks.

use std::path::{Path, PathBuf};

/// Try to resolve a filename in the mods directory, trying the exact name
/// first, then a percent-encoded variant (for legacy files saved with
/// literal `%2B` etc.), then with `.disabled` appended.
fn resolve_mod_file(dir: &Path, filename: &str) -> Option<PathBuf> {
    // 1. Exact match
    let p = dir.join(filename);
    if p.exists() {
        return Some(p);
    }
    // 2. Re-encode special characters (legacy files where %2B was literal)
    let encoded: String = percent_encoding::utf8_percent_encode(
        filename,
        percent_encoding::NON_ALPHANUMERIC,
    )
    .collect();
    let p2 = dir.join(&encoded);
    if p2.exists() {
        return Some(p2);
    }
    // 3. Append .disabled / .jar.disabled
    for suffix in [".disabled", ".jar.disabled"] {
        let p3 = dir.join(format!("{filename}{suffix}"));
        if p3.exists() {
            return Some(p3);
        }
    }
    None
}

use std::collections::HashMap;

use crate::error::Error;
use crate::world as wm;

use super::server::ManagedServer;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Information about a single mod or plugin file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModInfo {
    pub filename: String,
    pub name: String,
    pub enabled: bool,
    pub size_bytes: u64,
    pub size_human: String,
    pub last_modified: String,
    /// The original download URL, if known. Persisted in manifest.json.
    pub download_url: Option<String>,
}

/// Information about a generated `.mrpack` modpack archive.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModpackInfo {
    pub name: String,
    pub version: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub include_count: usize,
}

// ---------------------------------------------------------------------------
// impl ManagedServer — mod / plugin methods
// ---------------------------------------------------------------------------

impl ManagedServer {
    /// The directory where mods/plugins are stored for this server.
    pub fn mods_dir(&self) -> PathBuf {
        self.config
            .server_dir
            .join(match self.provider.to_lowercase().as_str() {
                "fabric" | "forge" | "neoforge" => "mods",
                _ => "plugins",
            })
    }

    /// Human-readable type: `"mod"` or `"plugin"`.
    pub fn mod_type(&self) -> &str {
        match self.provider.to_lowercase().as_str() {
            "fabric" | "forge" | "neoforge" => "mod",
            _ => "plugin",
        }
    }

    // ── Download-URL manifest ───────────────────────────────────

    /// Path to the manifest JSON that tracks original download URLs.
    fn mods_manifest_path(&self) -> PathBuf {
        self.mods_dir().join("manifest.json")
    }

    /// Read the download-url manifest, returning a map of filename → url.
    fn read_mods_manifest(&self) -> HashMap<String, String> {
        let path = self.mods_manifest_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok())
            .unwrap_or_default()
    }

    /// Write the download-url manifest.
    fn write_mods_manifest(&self, manifest: &HashMap<String, String>) -> Result<(), Error> {
        let path = self.mods_manifest_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::other(e.to_string()))?;
        }
        let data = serde_json::to_string_pretty(manifest)
            .map_err(|e| Error::other(e.to_string()))?;
        std::fs::write(&path, data).map_err(|e| Error::other(e.to_string()))
    }

    /// List all mod/plugin files in the server's mods/plugins directory.
    pub fn list_mods(&self) -> Result<Vec<ModInfo>, Error> {
        let dir = self.mods_dir();
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        let manifest = self.read_mods_manifest();
        let mut items: Vec<ModInfo> = std::fs::read_dir(&dir)
            .map_err(|e| Error::other(e.to_string()))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                (n.ends_with(".jar") || n.ends_with(".jar.disabled")) && e.path().is_file()
            })
            .map(|e| {
                let fn_ = e.file_name().to_string_lossy().to_string();
                let enabled = !fn_.ends_with(".disabled");
                let name = fn_
                    .strip_suffix(".jar")
                    .or_else(|| fn_.strip_suffix(".jar.disabled"))
                    .unwrap_or(&fn_)
                    .to_string();
                // Strip .disabled suffix when looking up the manifest
                let manifest_key = fn_.strip_suffix(".disabled").unwrap_or(&fn_).to_string();
                let download_url = manifest.get(&manifest_key).cloned();
                let meta = e.metadata().ok();
                ModInfo {
                    filename: fn_,
                    name,
                    enabled,
                    size_bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    size_human: wm::human_size(
                        meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    ),
                    last_modified: meta
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            let dt: chrono::DateTime<chrono::Utc> = t.into();
                            dt.to_rfc3339()
                        })
                        .unwrap_or_default(),
                    download_url,
                }
            })
            .collect();
        items.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(items)
    }

    /// Download and install a mod/plugin from a URL into the server.
    pub async fn install_mod(&self, url: &str, filename: &str) -> Result<ModInfo, Error> {
        let dir = self.mods_dir();
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| Error::other(e.to_string()))?;
        let dest = dir.join(filename);
        mc_server_installer::download(url, &dest).await?;
        // Persist the download URL in the manifest
        let mut manifest = self.read_mods_manifest();
        manifest.insert(filename.to_string(), url.to_string());
        self.write_mods_manifest(&manifest)?;
        let meta = tokio::fs::metadata(&dest)
            .await
            .map_err(|e| Error::other(e.to_string()))?;
        Ok(ModInfo {
            filename: filename.to_string(),
            name: filename
                .strip_suffix(".jar")
                .unwrap_or(filename)
                .to_string(),
            enabled: true,
            size_bytes: meta.len(),
            size_human: wm::human_size(meta.len()),
            last_modified: meta
                .modified()
                .ok()
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_default(),
            download_url: Some(url.to_string()),
        })
    }

    /// Delete a mod/plugin file from the server.
    pub fn delete_mod(&self, filename: &str) -> Result<(), Error> {
        let dir = self.mods_dir();
        let path = resolve_mod_file(&dir, filename)
            .ok_or_else(|| Error::other(format!("'{filename}' not found")))?;
        std::fs::remove_file(&path).map_err(|e| Error::other(e.to_string()))?;
        // Clean up the manifest entry
        let manifest_key = filename.strip_suffix(".disabled").unwrap_or(filename).to_string();
        let mut manifest = self.read_mods_manifest();
        if manifest.remove(&manifest_key).is_some() {
            self.write_mods_manifest(&manifest)?;
        }
        Ok(())
    }

    /// Enable or disable a mod/plugin by renaming with/without `.disabled`.
    ///
    /// `filename` may or may not already have a `.disabled` suffix — the
    /// function normalises it automatically.
    pub fn toggle_mod(&self, filename: &str, enabled: bool) -> Result<ModInfo, Error> {
        let dir = self.mods_dir();
        // Normalise: strip any trailing .disabled so we always work with
        // the base name, then add/remove the suffix as requested.
        let base = filename.strip_suffix(".disabled").unwrap_or(filename);
        let (src, dst) = if enabled {
            (format!("{base}.disabled"), base.to_string())
        } else {
            (base.to_string(), format!("{base}.disabled"))
        };
        // Resolve source using the helper (handles legacy %-encoded names)
        let sp = resolve_mod_file(&dir, &src)
            .ok_or_else(|| Error::other(format!("'{src}' not found")))?;
        let dp = dir.join(&dst);
        std::fs::rename(&sp, &dp).map_err(|e| Error::other(e.to_string()))?;
        let meta = std::fs::metadata(&dp).map_err(|e| Error::other(e.to_string()))?;
        let name = filename
            .strip_suffix(".jar")
            .unwrap_or(filename)
            .to_string();
        // Preserve the download URL from the manifest
        let manifest = self.read_mods_manifest();
        let download_url = manifest.get(base).cloned();
        Ok(ModInfo {
            filename: dst,
            name,
            enabled,
            size_bytes: meta.len(),
            size_human: wm::human_size(meta.len()),
            last_modified: meta
                .modified()
                .ok()
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_default(),
            download_url,
        })
    }

    // ── Modpack generation ────────────────────────────────────────

    /// Return the base directory where generated modpacks are stored.
    pub fn modpack_dir(&self) -> PathBuf {
        PathBuf::from("./data/modpacks").join(&self.handle.id)
    }

    /// Return the path to the most recent `.mrpack` file, if any.
    pub fn modpack_path(&self) -> Option<PathBuf> {
        let dir = self.modpack_dir();
        if !dir.is_dir() {
            return None;
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |x| x == "mrpack"))
            .map(|e| e.path())
            .collect();
        files.sort_by(|a, b| {
            b.metadata()
                .and_then(|m| m.modified())
                .ok()
                .cmp(&a.metadata().and_then(|m| m.modified()).ok())
        });
        files.into_iter().next()
    }

    /// Generate a Modrinth `.mrpack` modpack archive from the enabled
    /// mods (or a specific subset).
    ///
    /// Mods are listed in `files[]` with their original download URLs
    /// so that launchers can download them directly. No mod jars are
    /// embedded in the zip — the mrpack is a lightweight metadata
    /// archive that references external download sources.
    pub fn generate_modpack(
        &self,
        name: &str,
        version: &str,
        include: &[String],
    ) -> Result<ModpackInfo, Error> {
        use std::io::Write;
        use sha2::Digest;

        /// Replace characters that are illegal in Windows paths.
        fn sanitize_filename(s: &str) -> String {
            s.chars()
                .map(|c| {
                    if c == '\\' || c == '/' || c == ':' || c == '*'
                        || c == '?' || c == '"' || c == '<' || c == '>'
                        || c == '|'
                        || (c as u32) < 32
                    {
                        '_'
                    } else {
                        c
                    }
                })
                .collect()
        }

        let mods = self.list_mods()?;
        let dir = self.mods_dir();
        let selected: Vec<&ModInfo> = if include.is_empty() {
            mods.iter().filter(|m| m.enabled).collect()
        } else {
            mods.iter()
                .filter(|m| include.contains(&m.filename))
                .collect()
        };
        if selected.is_empty() {
            return Err(Error::other("No mods selected"));
        }

        // Every selected mod must have a tracked download URL, otherwise
        // launchers won't be able to download the file and the pack will
        // be broken.  Mods installed before the manifest system (v0.2+)
        // need to be reinstalled to register their URL.
        let missing_urls: Vec<&&ModInfo> = selected.iter().filter(|m| m.download_url.is_none()).collect();
        if !missing_urls.is_empty() {
            let names: Vec<&str> = missing_urls.iter().map(|m| m.filename.as_str()).collect();
            return Err(Error::other(format!(
                "Missing download URLs for mods: {}. Reinstall them to register their URLs.",
                names.join(", ")
            )));
        }
        let mt = if self.mod_type() == "mod" {
            "mods"
        } else {
            "plugins"
        };

        // Build files[] entries with download URLs, hashes, and file size
        let files: Vec<serde_json::Value> = selected
            .iter()
            .map(|m| {
                let data = std::fs::read(dir.join(&m.filename)).unwrap_or_default();
                // Sanitise the filename to remove Windows-illegal characters
                let safe_filename = sanitize_filename(&m.filename);
                let path = format!("{}/{}", mt, safe_filename);
                let downloads: Vec<String> = m
                    .download_url
                    .as_ref()
                    .map(|u| vec![u.clone()])
                    .unwrap_or_default();
                serde_json::json!({
                    "path": path,
                    "downloads": downloads,
                    "hashes": {
                        "sha1": sha1_smol::Sha1::from(&data).digest().to_string(),
                        "sha512": format!("{:x}", sha2::Sha512::digest(&data)),
                    },
                    "fileSize": data.len() as u64,
                })
            })
            .collect();

        // Dependencies: minecraft version + optional loader with a real version.
        // We detect the actual loader version from the server's installed files
        // rather than using wildcards/range strings that contain characters illegal
        // in Windows paths (e.g. "*", ">", "=") — some launchers (PCL) use the
        // version string literally in file paths.
        let mut deps = serde_json::json!({"minecraft": self.version});
        if let Some((key, version)) = self.detect_loader_version() {
            deps.as_object_mut()
                .map(|o| o.insert(key, serde_json::Value::String(version)));
        }

        let idx = serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": sanitize_filename(version),
            "name": name,
            "summary": format!("Server modpack for {} {}", self.provider, self.version),
            "files": files,
            "dependencies": deps,
        });

        let out_dir = self.modpack_dir();
        std::fs::create_dir_all(&out_dir).map_err(|e| Error::other(e.to_string()))?;
        let safe_name = sanitize_filename(name).replace(' ', "-").to_lowercase();
        let safe_ver = sanitize_filename(version);
        let op = out_dir.join(format!("{}-{}-{}.mrpack", self.handle.id, safe_name, safe_ver));
        let file = std::fs::File::create(&op).map_err(|e| Error::other(e.to_string()))?;
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // Only modrinth.index.json goes in the zip — no overrides/
        zip.start_file("modrinth.index.json", opts)
            .map_err(|e| Error::other(e.to_string()))?;
        zip.write_all(
            serde_json::to_string_pretty(&idx)
                .map_err(|e| Error::other(e.to_string()))?
                .as_bytes(),
        )
        .map_err(|e| Error::other(e.to_string()))?;

        zip.finish().map_err(|e| Error::other(e.to_string()))?;
        let size = std::fs::metadata(&op)
            .map_err(|e| Error::other(e.to_string()))?
            .len();
        Ok(ModpackInfo {
            name: name.to_string(),
            version: version.to_string(),
            file_path: op.to_string_lossy().to_string(),
            size_bytes: size,
            include_count: selected.len(),
        })
    }
}
