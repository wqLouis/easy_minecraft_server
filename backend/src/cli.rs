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
    Serve,
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
}

/// Dispatch CLI commands.
pub async fn dispatch(cli: Cli, pool: SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = settings::default_settings_path();
    let blacklist_path = blacklist::default_blacklist_path();

    match cli.command {
        Commands::Serve => {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .init();
            crate::serve::serve(pool, settings_path, blacklist_path).await
        }
        Commands::CreateSudo { email } => create_sudo(&pool, &email).await,
        Commands::ListUsers => list_users(&pool).await,
        Commands::BanStatus => ban_status(&blacklist_path).await,
        Commands::Unban { ip } => unban(&blacklist_path, &ip).await,
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
