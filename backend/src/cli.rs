use std::path::PathBuf;

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use chrono::Utc;
use clap::{Parser, Subcommand};
use sqlx::SqlitePool;

use crate::auth::generate_api_key;
use crate::blacklist;
use crate::models::User;
use crate::settings;

/// Backend CLI — host-machine administration.
#[derive(Parser, Debug)]
#[command(name = "backend", version, about = "Minecraft server backend")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the HTTP API server
    Serve {
        /// Detach from terminal and run in background
        #[arg(long, default_value_t = false)]
        daemon: bool,
    },
    /// Create a new sudo (admin) user directly in the database.
    CreateSudo {
        /// Email address for the new sudo user
        #[arg(short, long)]
        email: String,
    },
    /// List all users in the database.
    ListUsers,
    /// View fail2ban / blacklist status.
    BanStatus,
    /// Remove an IP from the blacklist.
    Unban {
        /// IP address to unban
        ip: String,
    },
    /// Drop all tables and reset the database to a clean state.
    /// Also resets settings and blacklist files.
    ResetDb,
    /// Generate and install a systemd user service for the backend.
    ///
    /// Default path: ~/.config/systemd/user/easymc-server.service
    InstallService {
        /// Output path (default: ~/.config/systemd/user/easymc-server.service)
        #[arg(short, long)]
        output: Option<String>,
    },
}

/// Dispatch CLI commands.
pub async fn dispatch(cli: Cli, pool: SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = settings::default_settings_path();
    let blacklist_path = blacklist::default_blacklist_path();

    match cli.command {
        Commands::Serve { daemon } => {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .init();
            if daemon {
                daemonize()?;
            }
            crate::serve::serve(pool, settings_path, blacklist_path).await
        }
        Commands::CreateSudo { email } => create_sudo(&pool, &email).await,
        Commands::ListUsers => list_users(&pool).await,
        Commands::BanStatus => ban_status(&blacklist_path).await,
        Commands::Unban { ip } => unban(&blacklist_path, &ip).await,
        Commands::ResetDb => reset_db(&pool, &settings_path, &blacklist_path).await,
        Commands::InstallService { output } => {
            let path = output.unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                format!("{home}/.config/systemd/user/easymc-server.service")
            });
            install_service(&path, &settings_path).await
        }
    }
}

// ── Create sudo ────────────────────────────────────────────────────

async fn create_sudo(pool: &SqlitePool, email: &str) -> Result<(), Box<dyn std::error::Error>> {
    let email = email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        eprintln!("error: invalid email address");
        std::process::exit(1);
    }

    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(pool)
        .await?;

    if existing > 0 {
        eprintln!("error: email '{}' is already registered", email);
        std::process::exit(1);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let api_key = generate_api_key();

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let api_key_hash = argon2
        .hash_password(api_key.as_bytes(), &salt)
        .map_err(|e| format!("hashing error: {}", e))?
        .to_string();

    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    sqlx::query(
        "INSERT INTO users (id, email, api_key_hash, is_sudoer, created_at, updated_at) \
         VALUES (?, ?, ?, 1, ?, ?)",
    )
    .bind(&id)
    .bind(&email)
    .bind(&api_key_hash)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    println!("✅ Sudo user created successfully!");
    println!("   Email:   {}", email);
    println!("   API key: {}", api_key);
    println!();
    println!("⚠️  Save this API key — it will not be shown again.");

    Ok(())
}

// ── List users ─────────────────────────────────────────────────────

async fn list_users(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at ASC")
        .fetch_all(pool)
        .await?;

    if users.is_empty() {
        println!("No users found.");
        return Ok(());
    }

    println!("{:<10} {:<30} {:<6} {}", "ID", "Email", "Sudo", "Created");
    println!("{}", "-".repeat(75));
    for user in &users {
        println!(
            "{:<10} {:<30} {:<6} {}",
            &user.id[..8],
            user.email,
            if user.is_sudoer { "yes" } else { "no" },
            user.created_at,
        );
    }

    Ok(())
}

// ── Ban status ─────────────────────────────────────────────────────

async fn ban_status(blacklist_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let app_settings = settings::load_settings(&settings::default_settings_path());
    println!("=== Fail2ban Configuration ===");
    println!(
        "  Max failed attempts before permanent blacklist: {}",
        app_settings.fail2ban_max_attempts
    );
    println!();

    let ips = blacklist::load_blacklist(blacklist_path)?;
    if ips.is_empty() {
        println!("No blacklisted IPs.");
    } else {
        println!("=== Blacklisted IPs (data/blacklist.json) ===");
        for ip in &ips {
            println!("  - {}", ip);
        }
    }

    Ok(())
}

// ── Daemonize ─────────────────────────────────────────────────────

/// Re-launch the server detached from the terminal via nohup.
fn daemonize() -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let log = std::fs::File::create("server.log")?;

    let child = std::process::Command::new("nohup")
        .arg(&exe)
        .arg("serve")
        .stdout(log.try_clone()?)
        .stderr(log)
        .stdin(std::process::Stdio::null())
        .spawn()?;

    println!("✅ Server started in background (PID: {})", child.id());
    println!("   Logs: ./server.log");
    println!("   Stop: kill {}", child.id());
    std::process::exit(0);
}

// ── Install systemd service ───────────────────────────────────────

async fn install_service(
    output: &str,
    settings_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let cwd = std::env::current_dir()?;
    let settings_abs = std::fs::canonicalize(settings_path)
        .unwrap_or_else(|_| cwd.join(settings_path));
    let data_dir = settings_abs.parent().unwrap_or(&cwd);

    let unit = format!(
        r#"[Unit]
Description=Minecraft Server Backend API
After=network.target

[Service]
Type=simple
ExecStart={} serve
WorkingDirectory={}
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info
StandardOutput=append:{}/server.log
StandardError=append:{}/server.log

[Install]
WantedBy=multi-user.target
"#,
        exe.display(),
        cwd.display(),
        data_dir.display(),
        data_dir.display(),
    );

    let path = std::path::Path::new(output);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &unit)?;

    let service_name = path.file_stem().unwrap().to_string_lossy().to_string();
    let is_user = output.contains("systemd/user/");
    let prefix = if is_user { "" } else { "sudo " };
    let user_flag = if is_user { " --user" } else { "" };

    println!("✅ Systemd{} service written to {}",
        if is_user { " user" } else { "" },
        path.display(),
    );
    println!();
    println!("   To enable and start:");
    println!("     {prefix}systemctl{user_flag} daemon-reload");
    println!("     {prefix}systemctl{user_flag} enable --now {service_name}");
    println!();
    println!("   To view logs:");
    println!("     {prefix}journalctl{user_flag} -u {service_name} -f");
    println!();
    println!("   To stop:");
    println!("     {prefix}systemctl{user_flag} stop {service_name}");

    Ok(())
}

// ── Reset DB ──────────────────────────────────────────────────────

async fn reset_db(
    pool: &SqlitePool,
    settings_path: &PathBuf,
    blacklist_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    eprint!("⚠️  This will delete ALL users and reset settings. Continue? [y/N] ");
    std::io::Write::flush(&mut std::io::stderr())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Aborted.");
        return Ok(());
    }

    // Drop the users table
    sqlx::query("DROP TABLE IF EXISTS users")
        .execute(pool)
        .await?;
    println!("🗑️  Dropped `users` table.");

    // Re-run migrations (recreates the table)
    crate::db::run_migrations(pool).await?;
    println!("✅ Recreated `users` table.");

    // Reset settings file
    let default_settings = crate::settings::AppSettings::default();
    crate::settings::save_settings(settings_path, &default_settings)?;
    println!("✅ Reset settings to defaults.");

    // Reset blacklist
    blacklist::save_blacklist(blacklist_path, &Vec::<String>::new())?;
    println!("✅ Cleared blacklist.");

    println!();
    println!("📋 Database reset complete. Use `backend create-sudo` to create a new admin user.");

    Ok(())
}

// ── Unban ──────────────────────────────────────────────────────────

async fn unban(blacklist_path: &PathBuf, ip: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut ips = blacklist::load_blacklist(blacklist_path)?;
    let existed = ips.iter().any(|x| x == ip);
    ips.retain(|x| x != ip);
    blacklist::save_blacklist(blacklist_path, &ips)?;

    if existed {
        println!("✅ Removed {} from blacklist.", ip);
    } else {
        println!("ℹ️  {} was not in the blacklist.", ip);
    }

    Ok(())
}
