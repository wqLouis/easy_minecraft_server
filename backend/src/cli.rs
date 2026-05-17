use crate::cmd;
use crate::ip_ban;
use crate::settings;
use clap::{Parser, Subcommand};
use sqlx::SqlitePool;

#[derive(Parser, Debug)]
#[command(name = "eazymc-backend", version, about = "Minecraft server backend")]
pub struct Cli {
    #[arg(long, default_value = "info", global = true)]
    pub log_level: String,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the HTTP API server
    Serve {
        #[arg(long)]
        daemon: bool,
        /// Store all server files in RAM (system temp dir).
        #[arg(long)]
        tmpfs: bool,
        /// Size hint for tmpfs (e.g. "2G").
        #[arg(long)]
        tmpfs_size: Option<String>,
        /// Tmpfs directory (default: system temp + /easymc).
        #[arg(long)]
        tmpfs_path: Option<String>,
    },
    /// Create a sudo user directly in the database.
    CreateSudo {
        #[arg(short, long)]
        username: String,
    },
    /// List all users.
    ListUsers,
    /// View fail2ban status.
    BanStatus,
    /// Remove an IP from the blacklist.
    Unban { ip: String },
    /// Reset database, settings, and blacklist.
    ResetDb,
    /// Install a systemd service.
    InstallService {
        #[arg(short, long)]
        output: Option<String>,
    },
    /// List IP whitelist entries.
    WhitelistList,
    /// Clear IP whitelist for a user.
    WhitelistClear { user: String },
}

pub async fn dispatch(
    cli: Cli,
    pool: SqlitePool,
    database_url: String,
) -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cli.log_level))
        .init();
    let settings_path = settings::default_settings_path();
    let blacklist_path = ip_ban::default_blacklist_path();
    match cli.command {
        Commands::Serve {
            daemon,
            tmpfs,
            tmpfs_size,
            tmpfs_path,
        } => {
            if daemon {
                daemonize(tmpfs, tmpfs_size.clone(), tmpfs_path.clone())?;
            }
            crate::serve::serve(
                pool,
                settings_path,
                blacklist_path,
                tmpfs,
                tmpfs_size,
                tmpfs_path,
                database_url,
            )
            .await
        }
        Commands::CreateSudo { username } => cmd::create_sudo(&pool, &username).await,
        Commands::ListUsers => cmd::list_users(&pool).await,
        Commands::BanStatus => cmd::ban_status(&blacklist_path).await,
        Commands::Unban { ip } => cmd::unban(&blacklist_path, &ip).await,
        Commands::WhitelistList => cmd::whitelist_list(&pool).await,
        Commands::WhitelistClear { user } => cmd::whitelist_clear(&pool, &user).await,
        Commands::ResetDb => cmd::reset_db(&pool, &settings_path, &blacklist_path).await,
        Commands::InstallService { output } => {
            let path = output.unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                format!("{home}/.config/systemd/user/easymc-server.service")
            });
            cmd::install_service(&path, &settings_path).await
        }
    }
}

fn daemonize(
    tmpfs: bool,
    tmpfs_size: Option<String>,
    tmpfs_path: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let log = std::fs::File::create("server.log")?;
    let mut c = std::process::Command::new("nohup");
    c.arg(&exe).arg("serve");
    if tmpfs {
        c.arg("--tmpfs");
    }
    if let Some(ref s) = tmpfs_size {
        c.arg("--tmpfs-size").arg(s);
    }
    if let Some(ref p) = tmpfs_path {
        c.arg("--tmpfs-path").arg(p);
    }
    let child = c
        .stdout(log.try_clone()?)
        .stderr(log)
        .stdin(std::process::Stdio::null())
        .spawn()?;
    println!("✅ Server started in background (PID: {})", child.id());
    println!("   Logs: ./server.log\n   Stop: kill {}", child.id());
    std::process::exit(0);
}
