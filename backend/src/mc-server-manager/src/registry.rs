use crate::error::Error;
use crate::instance::ServerConfig;
use crate::managers::{ManagedServer, ServerHandle};
use crate::world::{dir_size, human_size};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Try to detect the server provider from a directory name or files inside.
fn detect_provider(dir: &Path) -> Option<String> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if dir.join(".fabric").is_dir() {
        return Some("fabric".into());
    }
    if dir.join("fabric-server-launch.jar").exists() {
        return Some("fabric".into());
    }
    if name.contains("fabric") {
        return Some("fabric".into());
    }
    if name.contains("forge") || name.contains("neoforge") {
        return Some("forge".into());
    }
    if name.contains("paper") {
        return Some("paper".into());
    }
    if name.contains("purpur") {
        return Some("purpur".into());
    }
    None
}

/// Try to detect the Minecraft version from a directory name
/// (e.g. "fabric-1-20-1" → "1.20.1").
fn detect_version_from_dir(dir: &Path) -> Option<String> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let separators: &[char] = &['.', '-', '_'];
    let parts: Vec<&str> = name.split(separators).collect();
    for i in 0..parts.len().saturating_sub(2) {
        if let (Ok(major), Ok(minor)) = (parts[i].parse::<u32>(), parts[i + 1].parse::<u32>()) {
            if major <= 3 {
                let patch = parts
                    .get(i + 2)
                    .and_then(|p| p.parse::<u32>().ok())
                    .map(|p| format!(".{p}"))
                    .unwrap_or_default();
                return Some(format!("{major}.{minor}{patch}"));
            }
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InstanceConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub version: String,
    pub java_path: String,
    pub min_memory: String,
    pub max_memory: String,
    pub jvm_args: Vec<String>,
    #[serde(default)]
    pub server_dir: String,
    #[serde(default)]
    pub jar_path: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct InstanceSummary {
    pub id: String,
    pub name: String,
    pub running: bool,
}
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

#[derive(Clone)]
pub struct ServerRegistry {
    instances: Arc<RwLock<HashMap<String, ManagedServer>>>,
    archive_root: PathBuf,
    /// Whether the registry was loaded from disk (vs empty).
    pub loaded: bool,
}

/// Name of the per-instance config file stored inside each server directory.
pub const INSTANCE_CONFIG_FILE: &str = ".instance.json";

impl ServerRegistry {
    /// Create a new registry, scanning `servers_dir` for existing instances.
    ///
    /// Each subdirectory containing a `.instance.json` file is loaded as an
    /// instance. Subdirectories without a config file are ignored (they can
    /// be imported later via [`import_servers_dir`](Self::import_servers_dir)).
    pub fn new(archive_root: PathBuf) -> Self {
        Self {
            instances: Arc::new(RwLock::new(HashMap::new())),
            archive_root,
            loaded: false,
        }
    }

    /// Load all instances by scanning `servers_dir` for `.instance.json` files.
    /// Returns the number of instances loaded.
    pub fn load_all(&self, servers_dir: &Path) -> Result<usize, Error> {
        let mut count = 0;
        if !servers_dir.is_dir() {
            return Ok(0);
        }
        for entry in std::fs::read_dir(servers_dir).map_err(|e| Error::other(e.to_string()))? {
            let entry = entry.map_err(|e| Error::other(e.to_string()))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let cfg_path = path.join(INSTANCE_CONFIG_FILE);
            if !cfg_path.is_file() {
                continue;
            }
            let data =
                std::fs::read_to_string(&cfg_path).map_err(|e| Error::other(e.to_string()))?;
            let cfg: InstanceConfig =
                serde_json::from_str(&data).map_err(|e| Error::other(e.to_string()))?;
            let mut m = self
                .instances
                .write()
                .map_err(|e| Error::other(e.to_string()))?;
            m.insert(cfg.id.clone(), cfg.to_managed());
            count += 1;
        }
        Ok(count)
    }

    /// Scan `servers_dir` for subdirectories that do NOT have a `.instance.json`
    /// yet, auto-detect their provider/version, and register them.
    pub fn import_servers_dir(&self, servers_dir: &Path) -> Result<usize, Error> {
        let mut imported = 0;
        if !servers_dir.is_dir() {
            return Ok(0);
        }
        let existing: Vec<String> = self
            .instances
            .read()
            .map_err(|e| Error::other(e.to_string()))?
            .keys()
            .cloned()
            .collect();
        for entry in std::fs::read_dir(servers_dir).map_err(|e| Error::other(e.to_string()))? {
            let entry = entry.map_err(|e| Error::other(e.to_string()))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().to_string();
            if existing.contains(&id) {
                continue;
            }
            // Skip if it already has a config file (would have been loaded by load_all)
            if path.join(INSTANCE_CONFIG_FILE).is_file() {
                continue;
            }
            let server_dir = path.to_string_lossy().to_string();
            let jar_path = path.join("server.jar").to_string_lossy().to_string();
            let provider = detect_provider(&path).unwrap_or_else(|| "vanilla".into());
            let version = detect_version_from_dir(&path).unwrap_or_else(|| "unknown".into());
            let cfg = InstanceConfig {
                id: id.clone(),
                name: id.clone(),
                provider,
                version,
                java_path: String::new(),
                min_memory: "1G".into(),
                max_memory: "4G".into(),
                jvm_args: vec![],
                server_dir,
                jar_path,
            };
            let mut m = self
                .instances
                .write()
                .map_err(|e| Error::other(e.to_string()))?;
            m.insert(id.clone(), cfg.to_managed());
            drop(m);
            self.save_one(&cfg)?;
            imported += 1;
            log::info!("Imported existing server '{}' from {}", id, path.display());
        }
        Ok(imported)
    }

    pub fn create(&self, config: InstanceConfig) -> Result<(), Error> {
        let s = config.to_managed();
        let mut m = self
            .instances
            .write()
            .map_err(|e| Error::other(e.to_string()))?;
        if m.contains_key(&config.id) {
            return Err(Error::other(format!("'{}' exists", config.id)));
        }
        m.insert(config.id.clone(), s);
        drop(m);
        self.save_one(&config)
    }
    fn sanitize_id(id: &str) -> Result<(), Error> {
        if id.is_empty()
            || id.contains('/')
            || id.contains('\\')
            || id.contains("..")
            || id.contains('~')
        {
            return Err(Error::other("Invalid instance ID"));
        }
        Ok(())
    }
    pub fn remove(&self, id: &str) -> Result<(), Error> {
        Self::sanitize_id(id)?;
        let (cfg, h) = self
            .get_info(id)
            .ok_or_else(|| Error::other(format!("'{id}' not found")))?;
        if h.is_running() {
            return Err(Error::other("Stop first"));
        }
        let src = PathBuf::from(&cfg.server_dir);
        let dst = self.archive_root.join(id);
        if src.exists() {
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p).map_err(|e| Error::other(e.to_string()))?;
            }
            std::fs::rename(&src, &dst).map_err(|e| Error::other(e.to_string()))?;
        }
        if dst.exists() {
            if let Ok(c) = serde_json::to_string_pretty(&cfg) {
                let _ = std::fs::write(dst.join(INSTANCE_CONFIG_FILE), c);
            }
        }
        // Remove the old .instance.json from the original location
        let old_cfg = PathBuf::from(&cfg.server_dir).join(INSTANCE_CONFIG_FILE);
        let _ = std::fs::remove_file(&old_cfg);
        {
            let mut m = self
                .instances
                .write()
                .map_err(|e| Error::other(e.to_string()))?;
            m.remove(id);
        }
        log::info!("Instance '{id}' archived");
        Ok(())
    }
    pub fn update_config(&self, id: &str, config: InstanceConfig) -> Result<(), Error> {
        let ns = config.to_managed();
        let mut m = self
            .instances
            .write()
            .map_err(|e| Error::other(e.to_string()))?;
        if !m.contains_key(id) {
            return Err(Error::other(format!("'{id}' not found")));
        }
        m.remove(id);
        m.insert(config.id.clone(), ns);
        drop(m);
        self.save_one(&config)
    }
    pub fn list_archived(&self) -> Vec<ArchivedSummary> {
        let mut a = vec![];
        let d = &self.archive_root;
        if !d.is_dir() {
            return a;
        }
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().to_string();
            let c: Option<InstanceConfig> =
                std::fs::read_to_string(e.path().join(INSTANCE_CONFIG_FILE))
                    .ok()
                    .and_then(|c| serde_json::from_str(&c).ok());
            let (n, p, v) = c
                .map(|x| (x.name, x.provider, x.version))
                .unwrap_or_else(|| (id.clone(), "unknown".into(), "unknown".into()));
            let sz = dir_size(&e.path()).unwrap_or(0);
            let at = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_default();
            a.push(ArchivedSummary {
                id,
                name: n,
                provider: p,
                version: v,
                archived_at: at,
                size_bytes: sz,
                size_human: human_size(sz),
            });
        }
        a.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
        a
    }
    pub fn restore_archived(&self, id: &str) -> Result<(), Error> {
        Self::sanitize_id(id)?;
        let ad = self.archive_root.join(id);
        if !ad.is_dir() {
            return Err(Error::other(format!("Archived '{id}' not found")));
        }
        let cfg: InstanceConfig = serde_json::from_str(
            &std::fs::read_to_string(ad.join(INSTANCE_CONFIG_FILE))
                .map_err(|e| Error::other(e.to_string()))?,
        )
        .map_err(|e| Error::other(e.to_string()))?;
        let dst = PathBuf::from(&cfg.server_dir);
        if dst.exists() {
            return Err(Error::other(format!("Target exists: {:?}", dst)));
        }
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p).map_err(|e| Error::other(e.to_string()))?;
        }
        std::fs::rename(&ad, &dst).map_err(|e| Error::other(e.to_string()))?;
        {
            let mut m = self
                .instances
                .write()
                .map_err(|e| Error::other(e.to_string()))?;
            m.insert(id.to_string(), cfg.to_managed());
        }
        self.save_one(&cfg)?;
        log::info!("Instance '{id}' restored");
        Ok(())
    }
    pub fn archive_root(&self) -> &Path {
        &self.archive_root
    }
    pub fn list(&self) -> Vec<InstanceSummary> {
        self.instances
            .read()
            .ok()
            .map(|m| {
                m.iter()
                    .map(|(id, s)| InstanceSummary {
                        id: id.clone(),
                        name: s.handle().name().to_string(),
                        running: s.handle().is_running(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn get_server(&self, id: &str) -> Result<ManagedServer, Error> {
        self.instances
            .read()
            .map_err(|e| Error::other(e.to_string()))?
            .get(id)
            .cloned()
            .ok_or_else(|| Error::other(format!("'{id}' not found")))
    }
    pub fn get_info(&self, id: &str) -> Option<(InstanceConfig, ServerHandle)> {
        let m = self.instances.read().ok()?;
        let s = m.get(id)?;
        Some((InstanceConfig::from_managed(s), s.handle().clone()))
    }
    pub async fn start(&self, id: &str) -> Result<(), Error> {
        let mut server = self.get_server(id)?;
        server.start().await?;
        {
            let mut m = self
                .instances
                .write()
                .map_err(|e| Error::other(e.to_string()))?;
            m.insert(id.to_string(), server);
        }
        // Save config after start (no config change, but keep .instance.json fresh)
        if let Some((cfg, _)) = self.get_info(id) {
            self.save_one(&cfg)?;
        }
        Ok(())
    }
    pub async fn stop(&self, id: &str) -> Result<(), Error> {
        self.get_server(id)?.stop().await
    }
    pub async fn kill(&self, id: &str) -> Result<(), Error> {
        self.get_server(id)?.kill().await
    }
    pub async fn stop_all(&self) -> usize {
        self.batch(|s| async move { s.stop().await.is_ok() }).await
    }
    pub async fn kill_all(&self) -> usize {
        self.batch(|s| async move { s.kill().await.is_ok() }).await
    }
    async fn batch<F, Fut>(&self, f: F) -> usize
    where
        F: Fn(ManagedServer) -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let ids: Vec<_> = self
            .instances
            .read()
            .ok()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let mut n = 0;
        for id in &ids {
            if let Ok(s) = self.get_server(id) {
                if f(s).await {
                    n += 1;
                }
            }
        }
        n
    }
    pub async fn send_command(&self, id: &str, cmd: &str) -> Result<(), Error> {
        self.get_server(id)?.send_command(cmd).await
    }

    /// Save a single instance's config to its own server directory.
    fn save_one(&self, cfg: &InstanceConfig) -> Result<(), Error> {
        let dir = PathBuf::from(&cfg.server_dir);
        std::fs::create_dir_all(&dir).map_err(|e| Error::other(e.to_string()))?;
        let data = serde_json::to_string_pretty(cfg)
            .map_err(|e| Error::other(e.to_string()))?;
        std::fs::write(dir.join(INSTANCE_CONFIG_FILE), data)
            .map_err(|e| Error::other(e.to_string()))
    }
}

pub fn instance_config_schema() -> serde_json::Value {
    serde_json::to_value(&schemars::schema_for!(InstanceConfig)).unwrap_or_default()
}

impl InstanceConfig {
    fn to_managed(&self) -> ManagedServer {
        ManagedServer::new(
            self.id.clone(),
            self.name.clone(),
            ServerConfig::new(
                &self.jar_path,
                &self.java_path,
                &self.min_memory,
                &self.max_memory,
                &self.server_dir,
            )
            .with_jvm_args(self.jvm_args.clone()),
            PathBuf::from(&self.server_dir).join("data"),
            self.provider.clone(),
            self.version.clone(),
        )
    }
    fn from_managed(s: &ManagedServer) -> Self {
        let c = s.config();
        Self {
            id: s.handle().id().to_string(),
            name: s.handle().name().to_string(),
            provider: s.provider().to_string(),
            version: s.version().to_string(),
            jar_path: c.jar_path.to_string_lossy().to_string(),
            java_path: c.java_path.to_string_lossy().to_string(),
            min_memory: c.min_memory.clone(),
            max_memory: c.max_memory.clone(),
            server_dir: c.server_dir.to_string_lossy().to_string(),
            jvm_args: c.jvm_args.clone(),
        }
    }
}
