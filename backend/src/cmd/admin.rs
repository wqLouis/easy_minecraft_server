//! Admin CLI commands (create-sudo, list-users, ban-status, etc.).
use std::path::PathBuf;
use argon2::{password_hash::{rand_core::OsRng, SaltString}, Argon2, PasswordHasher};
use chrono::{TimeZone, Utc};
use sqlx::SqlitePool;
use crate::auth::generate_token;
use crate::ip_ban;
use crate::models::{IpWhitelistEntry, User};
use crate::settings;

pub async fn create_sudo(pool: &SqlitePool, username: &str, expires_days: u64) -> Result<(), Box<dyn std::error::Error>> {
    let username = username.trim().to_lowercase();
    if username.is_empty() || username.len() < 2 { eprintln!("error: username must be at least 2 characters"); std::process::exit(1); }
    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = ?").bind(&username).fetch_one(pool).await?;
    if existing > 0 { eprintln!("error: username '{}' already exists", username); std::process::exit(1); }
    let id = uuid::Uuid::new_v4().to_string();
    let token = generate_token();
    let credential_input = format!("{token}{username}");
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let credential_hash = argon2.hash_password(credential_input.as_bytes(), &salt).map_err(|e| format!("hashing error: {}", e))?.to_string();
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let (expires_at, display_expiry): (Option<String>, String) = if expires_days == 0 {
        (None, "never".to_string())
    } else {
        let e = (Utc::now() + chrono::Duration::days(expires_days as i64))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        (Some(e.clone()), e)
    };
    sqlx::query("INSERT INTO users (id, username, api_key_hash, is_sudoer, created_at, updated_at, api_key_expires_at) VALUES (?, ?, ?, 1, ?, ?, ?)")
        .bind(&id).bind(&username).bind(&credential_hash).bind(&now).bind(&now).bind(&expires_at)
        .execute(pool).await?;
    println!("✅ Sudo user created!\n   Username: {}\n   Token:    {}\n   Expires:  {}\n\n🔐 Authenticate with: Authorization: Bearer {}:{}", username, token, display_expiry, username, token);
    println!("⚠️  Save these credentials — the token will not be shown again.");
    Ok(())
}

pub async fn list_users(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at ASC").fetch_all(pool).await?;
    if users.is_empty() { println!("No users found."); return Ok(()); }
    println!("{:<10} {:<30} {:<6} {}", "ID", "Username", "Sudo", "Created");
    println!("{}", "-".repeat(75));
    for u in &users { println!("{:<10} {:<30} {:<6} {}", &u.id[..8], u.username, if u.is_sudoer { "yes" } else { "no" }, u.created_at); }
    Ok(())
}

pub async fn ban_status(blacklist_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let app_settings = settings::load_settings(&settings::default_settings_path());
    println!("=== Fail2ban Configuration ===\n  Max failed attempts before permanent blacklist: {}\n", app_settings.fail2ban_max_attempts);
    let ips = ip_ban::load_blacklist(blacklist_path)?;
    if ips.is_empty() { println!("No blacklisted IPs."); } else { println!("=== Blacklisted IPs (data/blacklist.json) ==="); for ip in &ips { println!("  - {}", ip); } }
    Ok(())
}

pub async fn unban(blacklist_path: &PathBuf, ip: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut ips = ip_ban::load_blacklist(blacklist_path)?;
    let existed = ips.iter().any(|x| x == ip);
    ips.retain(|x| x != ip);
    ip_ban::save_blacklist(blacklist_path, &ips)?;
    println!("{}", if existed { format!("✅ Removed {} from blacklist.", ip) } else { format!("ℹ️  {} was not in the blacklist.", ip) });
    Ok(())
}

pub async fn whitelist_list(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let entries = sqlx::query_as::<_, IpWhitelistEntry>(
        "SELECT w.id, w.user_id, w.ip, w.updated_at FROM ip_whitelist w INNER JOIN users u ON u.id = w.user_id ORDER BY w.updated_at DESC"
    ).fetch_all(pool).await?;
    if entries.is_empty() { println!("No active IP whitelist entries."); return Ok(()); }
    println!("{:<10} {:<30} {:<20} {}", "User ID", "IP", "Last Active", "Status");
    println!("{}", "-".repeat(80));
    let now = chrono::Utc::now();
    let ttl_secs = 12 * 60 * 60;
    for entry in &entries {
        let updated = chrono::NaiveDateTime::parse_from_str(&entry.updated_at, "%Y-%m-%d %H:%M:%S")
            .ok().map(|dt| Utc.from_utc_datetime(&dt));
        let (status, ts) = match updated { Some(dt) => { let age = now - dt; if age.num_seconds() < ttl_secs { (format!("active ({}h left)", (ttl_secs - age.num_seconds()) / 3600), &entry.updated_at) } else { ("expired".into(), &entry.updated_at) } } None => ("unknown".into(), &entry.updated_at) };
        println!("{:<10} {:<30} {:<20} {}", &entry.user_id[..8], entry.ip, ts, status);
    }
    Ok(())
}

pub async fn whitelist_clear(pool: &SqlitePool, user: &str) -> Result<(), Box<dyn std::error::Error>> {
    let user_row = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ? OR username = ?").bind(user).bind(user).fetch_optional(pool).await?;
    let user_id = match user_row { Some(u) => u.id, None => { eprintln!("error: user '{}' not found", user); std::process::exit(1); } };
    let result = sqlx::query("DELETE FROM ip_whitelist WHERE user_id = ?").bind(&user_id).execute(pool).await?;
    println!("✅ Cleared {} whitelist entr{} for user {}", result.rows_affected(), if result.rows_affected() == 1 { "y" } else { "ies" }, &user_id[..8]);
    Ok(())
}

pub async fn reset_db(pool: &SqlitePool, settings_path: &PathBuf, blacklist_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    eprint!("⚠️  This will delete ALL users and reset settings. Continue? [y/N] ");
    std::io::Write::flush(&mut std::io::stderr())?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !input.trim().eq_ignore_ascii_case("y") { println!("Aborted."); return Ok(()); }
    sqlx::query("DROP TABLE IF EXISTS users").execute(pool).await?;
    println!("🗑️  Dropped `users` table.");
    crate::db::run_migrations(pool).await?;
    println!("✅ Recreated `users` table.");
    settings::save_settings(settings_path, &settings::AppSettings::default())?;
    println!("✅ Reset settings to defaults.");
    ip_ban::save_blacklist(blacklist_path, &Vec::<String>::new())?;
    println!("✅ Cleared blacklist.");
    println!("\n📋 Database reset complete. Use `eazymc-backend create-sudo --username <name>` to create a new admin user.");
    Ok(())
}
