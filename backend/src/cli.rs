use std::path::PathBuf;

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand};
use sqlx::SqlitePool;

use crate::auth::generate_token;
use crate::ip_ban;

use crate::models::{IpWhitelistEntry, User};
use crate::settings;

/// Backend CLI — host-machine administration.
#[derive(Parser, Debug)]
#[command(name = "eazymc-backend", version, about = "Minecraft server backend")]
pub struct Cli {
    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", global = true)]
    pub log_level: String,

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
        /// Username for the new sudo user
        #[arg(short, long)]
        username: String,
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
    /// List all active IP whitelist entries.
    WhitelistList,
    /// Clear IP whitelist entries for a specific user (by ID or username).
    WhitelistClear {
        /// User ID or username to clear
        user: String,
    },
}

/// Dispatch CLI commands.
pub async fn dispatch(cli: Cli, pool: SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    // Init logger once with the user-requested level
    let filter = cli.log_level.to_lowercase();
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(&filter),
    )
    .init();

    let settings_path = settings::default_settings_path();
    let blacklist_path = ip_ban::default_blacklist_path();

    match cli.command {
        Commands::Serve { daemon } => {
            if daemon {
                daemonize()?;
            }
            crate::serve::serve(pool, settings_path, blacklist_path).await
        }
        Commands::CreateSudo { username } => create_sudo(&pool, &username).await,
        Commands::ListUsers => list_users(&pool).await,
        Commands::BanStatus => ban_status(&blacklist_path).await,
        Commands::Unban { ip } => unban(&blacklist_path, &ip).await,
        Commands::WhitelistList => whitelist_list(&pool).await,
        Commands::WhitelistClear { user } => whitelist_clear(&pool, &user).await,
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

async fn create_sudo(pool: &SqlitePool, username: &str) -> Result<(), Box<dyn std::error::Error>> {
    let username = username.trim().to_lowercase();
    if username.is_empty() || username.len() < 2 {
        eprintln!("error: username must be at least 2 characters");
        std::process::exit(1);
    }

    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = ?")
        .bind(&username)
        .fetch_one(pool)
        .await?;

    if existing > 0 {
        eprintln!("error: username '{}' already exists", username);
        std::process::exit(1);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let token = generate_token();

    // Hash token + username together (both needed for auth)
    let credential_input = format!("{token}{username}");
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let credential_hash = argon2
        .hash_password(credential_input.as_bytes(), &salt)
        .map_err(|e| format!("hashing error: {}", e))?
        .to_string();

    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    sqlx::query(
        "INSERT INTO users (id, username, api_key_hash, is_sudoer, created_at, updated_at) VALUES (?, ?, ?, 1, ?, ?)",
    )
    .bind(&id).bind(&username).bind(&credential_hash).bind(&now).bind(&now)
    .execute(pool)
    .await?;

    println!("✅ Sudo user created successfully!");
    println!("   Username: {}", username);
    println!("   Token:    {}", token);
    println!();
    println!("🔐 Authenticate with: Authorization: Bearer {}:{}", username, token);
    println!("⚠️  Save these credentials — the token will not be shown again.");

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

    println!("{:<10} {:<30} {:<6} {}", "ID", "Username", "Sudo", "Created");
    println!("{}", "-".repeat(75));
    for user in &users {
        println!(
            "{:<10} {:<30} {:<6} {}",
            &user.id[..8],
            user.username,
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

    let ips = ip_ban::load_blacklist(blacklist_path)?;
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

// ── Whitelist list ───────────────────────────────────────────────

async fn whitelist_list(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let entries = sqlx::query_as::<_, IpWhitelistEntry>(
        r#"
        SELECT w.id, w.user_id, w.ip, w.updated_at
        FROM ip_whitelist w
        INNER JOIN users u ON u.id = w.user_id
        ORDER BY w.updated_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    if entries.is_empty() {
        println!("No active IP whitelist entries.");
        return Ok(());
    }

    println!("{:<10} {:<30} {:<20} {}", "User ID", "IP", "Last Active", "Status");
    println!("{}", "-".repeat(80));

    let now = chrono::Utc::now();
    let ttl_secs = 12 * 60 * 60;

    for entry in &entries {
        let updated = match chrono::NaiveDateTime::parse_from_str(
            &entry.updated_at,
            "%Y-%m-%d %H:%M:%S",
        ) {
            Ok(dt) => chrono::Utc.from_utc_datetime(&dt),
            Err(_) => {
                println!("{:<10} {:<30} {:<20} {}", &entry.user_id[..8], entry.ip, entry.updated_at, "unknown");
                continue;
            }
        };

        let age = now - updated;
        let status = if age.num_seconds() < ttl_secs {
            format!("active ({}h left)", (ttl_secs - age.num_seconds()) / 3600)
        } else {
            "expired".into()
        };

        println!(
            "{:<10} {:<30} {:<20} {}",
            &entry.user_id[..8],
            entry.ip,
            entry.updated_at,
            status
        );
    }

    Ok(())
}

// ── Whitelist clear ──────────────────────────────────────────────

async fn whitelist_clear(pool: &SqlitePool, user: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Try to find user by ID or username
    let user_row = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ? OR username = ?",
    )
    .bind(user)
    .bind(user)
    .fetch_optional(pool)
    .await?;

    let user_id = match user_row {
        Some(u) => u.id,
        None => {
            eprintln!("error: user '{}' not found", user);
            std::process::exit(1);
        }
    };

    let result = sqlx::query("DELETE FROM ip_whitelist WHERE user_id = ?")
        .bind(&user_id)
        .execute(pool)
        .await?;

    println!(
        "✅ Cleared {} whitelist entr{} for user {}",
        result.rows_affected(),
        if result.rows_affected() == 1 { "y" } else { "ies" },
        &user_id[..8]
    );

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
    ip_ban::save_blacklist(blacklist_path, &Vec::<String>::new())?;
    println!("✅ Cleared blacklist.");

    println!();
    println!("📋 Database reset complete. Use `eazymc-backend create-sudo --username <name>` to create a new admin user.");

    Ok(())
}

// ── Unban ──────────────────────────────────────────────────────────

async fn unban(blacklist_path: &PathBuf, ip: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut ips = ip_ban::load_blacklist(blacklist_path)?;
    let existed = ips.iter().any(|x| x == ip);
    ips.retain(|x| x != ip);
    ip_ban::save_blacklist(blacklist_path, &ips)?;

    if existed {
        println!("✅ Removed {} from blacklist.", ip);
    } else {
        println!("ℹ️  {} was not in the blacklist.", ip);
    }

    Ok(())
}
