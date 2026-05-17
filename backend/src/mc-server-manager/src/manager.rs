use crate::error::Error;
use crate::instance::{ServerConfig, ServerInstance};
use crate::log::LogManager;
use crate::player::PlayerTracker;
use crate::properties::ServerProperties;
use crate::version::parse_provider;
use crate::world::{self as wm, BackupEntry, HistoryEntry, WorldInfo};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, broadcast};

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

#[derive(Clone)]
pub struct ManagedServer {
    handle: ServerHandle,
    process: Arc<Mutex<Option<ServerInstance>>>,
    config: ServerConfig,
    provider: String,
    version: String,
}

impl ManagedServer {
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

    async fn reader_task(shared: ServerHandle, mut rx: tokio::sync::mpsc::Receiver<String>) {
        while let Some(line) = rx.recv().await {
            shared.log_manager.lock().await.push(line.clone());
            shared.player_tracker.lock().await.process_log_line(&line);
            let _ = shared.log_tx.send(line);
        }
        shared.running.store(false, Ordering::SeqCst);
    }

    pub async fn stop(&self) -> Result<(), Error> {
        let mut p = self.process.lock().await;
        if let Some(ref mut i) = *p {
            i.stop().await?;
        }
        self.handle.running.store(false, Ordering::SeqCst);
        Ok(())
    }
    pub async fn kill(&self) -> Result<(), Error> {
        let mut p = self.process.lock().await;
        if let Some(ref mut i) = *p {
            i.kill().await?;
        }
        self.handle.running.store(false, Ordering::SeqCst);
        Ok(())
    }
    pub async fn send_command(&self, cmd: &str) -> Result<(), Error> {
        let p = self.process.lock().await;
        p.as_ref()
            .ok_or_else(|| Error::other("Not running"))?
            .send_command(cmd)
    }
    pub fn handle(&self) -> &ServerHandle {
        &self.handle
    }
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
    pub fn provider(&self) -> &str {
        &self.provider
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn server_dir(&self) -> &Path {
        &self.config.server_dir
    }

    pub fn list_worlds(&self) -> Result<Vec<WorldInfo>, Error> {
        wm::scan_worlds(&self.config.server_dir)
    }
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
    pub fn extract_world_zip(&self, data: &[u8]) -> Result<String, Error> {
        wm::extract_world_zip(data, &self.config.server_dir)
    }
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
    pub fn backups_dir(&self) -> PathBuf {
        let p = PathBuf::from("./data/backups").join(&self.handle.id);
        let _ = std::fs::create_dir_all(&p);
        p
    }
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
                        size_human: wm::human_size(meta.as_ref().map(|m| m.len()).unwrap_or(0)),
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
    fn cmd_history_path(&self) -> PathBuf {
        PathBuf::from("./data/command_history").join(format!("{}.json", self.handle.id))
    }
    pub fn command_history(&self) -> Vec<HistoryEntry> {
        std::fs::read_to_string(self.cmd_history_path())
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    }
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
    pub fn mods_dir(&self) -> PathBuf {
        self.config
            .server_dir
            .join(match self.provider.to_lowercase().as_str() {
                "fabric" | "forge" | "neoforge" => "mods",
                _ => "plugins",
            })
    }
    pub fn mod_type(&self) -> &str {
        match self.provider.to_lowercase().as_str() {
            "fabric" | "forge" | "neoforge" => "mod",
            _ => "plugin",
        }
    }
    pub fn list_mods(&self) -> Result<Vec<ModInfo>, Error> {
        let dir = self.mods_dir();
        if !dir.is_dir() {
            return Ok(vec![]);
        }
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
                let meta = e.metadata().ok();
                ModInfo {
                    filename: fn_,
                    name,
                    enabled,
                    size_bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    size_human: wm::human_size(meta.as_ref().map(|m| m.len()).unwrap_or(0)),
                    last_modified: meta
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            let dt: chrono::DateTime<chrono::Utc> = t.into();
                            dt.to_rfc3339()
                        })
                        .unwrap_or_default(),
                }
            })
            .collect();
        items.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(items)
    }
    pub async fn install_mod(&self, url: &str, filename: &str) -> Result<ModInfo, Error> {
        let dir = self.mods_dir();
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| Error::other(e.to_string()))?;
        let dest = dir.join(filename);
        mc_server_installer::download(url, &dest).await?;
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
        })
    }
    pub fn delete_mod(&self, filename: &str) -> Result<(), Error> {
        let dir = self.mods_dir();
        let p = dir.join(filename);
        if !p.exists() {
            let dp = dir.join(format!("{filename}.disabled"));
            if dp.exists() {
                return std::fs::remove_file(&dp).map_err(|e| Error::other(e.to_string()));
            }
            return Err(Error::other(format!("'{filename}' not found")));
        }
        std::fs::remove_file(&p).map_err(|e| Error::other(e.to_string()))
    }
    pub fn toggle_mod(&self, filename: &str, enabled: bool) -> Result<ModInfo, Error> {
        let dir = self.mods_dir();
        let (src, dst) = if enabled {
            (format!("{filename}.disabled"), filename.to_string())
        } else {
            (filename.to_string(), format!("{filename}.disabled"))
        };
        let (sp, dp) = (dir.join(&src), dir.join(&dst));
        if !sp.exists() {
            return Err(Error::other(format!("'{src}' not found")));
        }
        std::fs::rename(&sp, &dp).map_err(|e| Error::other(e.to_string()))?;
        let meta = std::fs::metadata(&dp).map_err(|e| Error::other(e.to_string()))?;
        let name = filename
            .strip_suffix(".jar")
            .unwrap_or(filename)
            .to_string();
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
        })
    }
    pub fn modpack_dir(&self) -> PathBuf {
        PathBuf::from("./data/modpacks").join(&self.handle.id)
    }
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
    pub fn generate_modpack(
        &self,
        name: &str,
        version: &str,
        include: &[String],
    ) -> Result<ModpackInfo, Error> {
        use std::io::Write;
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
        let mt = if self.mod_type() == "mod" {
            "mods"
        } else {
            "plugins"
        };
        use sha2::Digest;
        let files: Vec<serde_json::Value> = selected.iter().map(|m| {
            let data = std::fs::read(dir.join(&m.filename)).unwrap_or_default();
            serde_json::json!({"path": format!("{}/{}", mt, m.filename), "downloads": [], "hashes": {"sha1": sha1_smol::Sha1::from(&data).digest().to_string(), "sha512": format!("{:x}", sha2::Sha512::digest(&data))}, "fileSize": data.len() as u64})
        }).collect();
        let mut deps = serde_json::json!({"minecraft": self.version});
        if let Some(k) = match self.provider.to_lowercase().as_str() {
            "fabric" => Some("fabric-loader"),
            "forge" => Some("forge"),
            "neoforge" => Some("neoforge"),
            "quilt" => Some("quilt-loader"),
            _ => None,
        } {
            deps.as_object_mut()
                .map(|o| o.insert(k.to_string(), serde_json::Value::String("0.0.0".into())));
        }
        let idx = serde_json::json!({"formatVersion": 1, "game": "minecraft", "versionId": version, "name": name, "summary": format!("Server modpack for {} {}", self.provider, self.version), "files": files, "dependencies": deps});
        let out_dir = self.modpack_dir();
        std::fs::create_dir_all(&out_dir).map_err(|e| Error::other(e.to_string()))?;
        let safe = name.replace(' ', "-").to_lowercase();
        let op = out_dir.join(format!("{}-{}-{}.mrpack", self.handle.id, safe, version));
        let file = std::fs::File::create(&op).map_err(|e| Error::other(e.to_string()))?;
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        zip.start_file("modrinth.index.json", opts)
            .map_err(|e| Error::other(e.to_string()))?;
        zip.write_all(
            serde_json::to_string_pretty(&idx)
                .map_err(|e| Error::other(e.to_string()))?
                .as_bytes(),
        )
        .map_err(|e| Error::other(e.to_string()))?;
        zip.add_directory(format!("overrides/{mt}"), opts)
            .map_err(|e| Error::other(e.to_string()))?;
        for m in &selected {
            let data =
                std::fs::read(dir.join(&m.filename)).map_err(|e| Error::other(e.to_string()))?;
            zip.start_file(format!("overrides/{}/{}", mt, m.filename), opts)
                .map_err(|e| Error::other(e.to_string()))?;
            zip.write_all(&data)
                .map_err(|e| Error::other(e.to_string()))?;
        }
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModInfo {
    pub filename: String,
    pub name: String,
    pub enabled: bool,
    pub size_bytes: u64,
    pub size_human: String,
    pub last_modified: String,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModpackInfo {
    pub name: String,
    pub version: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub include_count: usize,
}
