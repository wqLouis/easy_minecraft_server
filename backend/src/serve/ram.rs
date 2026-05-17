//! Tmpfs/RAM disk support. On `--tmpfs`, copies all server data to a
//! system temp directory (RAM-backed on most OSes) and syncs back on shutdown.
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct RamDisk {
    pub root: PathBuf,
    pub original_data_dir: PathBuf,
    pub original_servers_dir: PathBuf,
    pub data_dir: PathBuf,
    pub servers_dir: PathBuf,
}

impl RamDisk {
    pub fn setup(up: Option<&str>, data_dir: &Path, servers_dir: &Path) -> Result<Self, String> {
        let root = resolve(up);
        std::fs::create_dir_all(&root).map_err(|e| format!("create tmpfs: {e}"))?;
        let dd = root.join("data");
        let sd = root.join("servers");
        if data_dir.exists() && !dd.exists() {
            copy_dir(data_dir, &dd)?;
        } else if !data_dir.exists() {
            std::fs::create_dir_all(&dd).map_err(|e| format!("data dir: {e}"))?;
        }
        if servers_dir.exists() && !sd.exists() {
            copy_dir(servers_dir, &sd)?;
        }
        log::info!("Tmpfs ready at {}", root.display());
        Ok(Self {
            root,
            original_data_dir: data_dir.to_owned(),
            original_servers_dir: servers_dir.to_owned(),
            data_dir: dd,
            servers_dir: sd,
        })
    }
    pub fn sync_back(&self) -> Result<(), String> {
        if self.data_dir.exists() {
            replace(&self.data_dir, &self.original_data_dir)?;
        }
        if self.servers_dir.exists() {
            replace(&self.servers_dir, &self.original_servers_dir)?;
        }
        Ok(())
    }
    pub fn cleanup(&self) {
        if self.root.exists() {
            let _ = std::fs::remove_dir_all(&self.root);
            log::info!("Tmpfs {} cleaned", self.root.display());
        }
    }
}

fn resolve(up: Option<&str>) -> PathBuf {
    up.map_or_else(|| std::env::temp_dir().join("easymc"), PathBuf::from)
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("copy dir: {e}"))?;
    for e in std::fs::read_dir(src).map_err(|e| format!("read dir: {e}"))? {
        let e = e.map_err(|e| format!("entry: {e}"))?;
        let t = e.file_type().map_err(|e| format!("type: {e}"))?;
        let sp = e.path();
        let dp = dst.join(e.file_name());
        if t.is_dir() {
            copy_dir(&sp, &dp)?;
        } else {
            std::fs::copy(&sp, &dp).map_err(|e| format!("copy file: {e}"))?;
        }
    }
    Ok(())
}

fn replace(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| format!("rm {dst:?}: {e}"))?;
    }
    copy_dir(src, dst)
}
