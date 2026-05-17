use crate::error::Error;
use crate::instance::ServerConfig;
use crate::managers::{ManagedServer, ServerHandle};
use crate::world::{dir_size, human_size};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    instances: Vec<InstanceConfig>,
}

#[derive(Clone)]
pub struct ServerRegistry {
    instances: Arc<RwLock<HashMap<String, ManagedServer>>>,
    config_path: PathBuf,
    archive_root: PathBuf,
}

impl ServerRegistry {
    pub fn new(config_path: PathBuf) -> Self {
        let instances = Arc::new(RwLock::new(HashMap::new()));
        let ar = config_path
            .parent()
            .map(|p| p.join("_archived"))
            .unwrap_or_else(|| PathBuf::from("./data/_archived"));
        let r = Self {
            instances: instances.clone(),
            config_path,
            archive_root: ar,
        };
        if let Ok(s) = r.load() {
            let mut m = instances.write().unwrap();
            for c in s.instances {
                m.insert(c.id.clone(), c.to_managed());
            }
        }
        r
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
        self.save()
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
                let _ = std::fs::write(dst.join(".instance.json"), c);
            }
        }
        {
            let mut m = self
                .instances
                .write()
                .map_err(|e| Error::other(e.to_string()))?;
            m.remove(id);
        }
        self.save()?;
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
        m.insert(config.id, ns);
        drop(m);
        self.save()
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
                std::fs::read_to_string(e.path().join(".instance.json"))
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
            &std::fs::read_to_string(ad.join(".instance.json"))
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
        self.save()?;
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
        self.save()
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
    fn load(&self) -> Result<Saved, Error> {
        match std::fs::read_to_string(&self.config_path) {
            Ok(c) => serde_json::from_str(&c).map_err(|e| Error::other(format!("parse: {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Saved { instances: vec![] }),
            Err(e) => Err(Error::other(format!("read: {e}"))),
        }
    }
    fn save(&self) -> Result<(), Error> {
        let configs: Vec<InstanceConfig> = self
            .instances
            .read()
            .map_err(|e| Error::other(e.to_string()))?
            .values()
            .map(|s| InstanceConfig::from_managed(s))
            .collect();
        let c = serde_json::to_string_pretty(&Saved { instances: configs })
            .map_err(|e| Error::other(e.to_string()))?;
        if let Some(p) = self.config_path.parent() {
            std::fs::create_dir_all(p).map_err(|e| Error::other(e.to_string()))?;
        }
        std::fs::write(&self.config_path, c).map_err(|e| Error::other(e.to_string()))?;
        Ok(())
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
