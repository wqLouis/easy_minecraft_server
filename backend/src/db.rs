//! SQLite pool initialization and migrations.
use sqlx::SqlitePool;
use std::path::Path;

pub async fn init_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    if let Some(path) = url
        .strip_prefix("sqlite:")
        .and_then(|s| s.split('?').next())
    {
        if let Some(parent) = Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let pool = SqlitePool::connect(url).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY NOT NULL, username TEXT NOT NULL UNIQUE, api_key_hash TEXT NOT NULL, is_sudoer INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT (datetime('now')), updated_at TEXT NOT NULL DEFAULT (datetime('now')), api_key_expires_at TEXT)").execute(pool).await?;
    // Add api_key_expires_at to existing tables (safe no-op if column already exists)
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN api_key_expires_at TEXT")
        .execute(pool)
        .await;
    sqlx::query("CREATE TABLE IF NOT EXISTS ip_whitelist (id TEXT PRIMARY KEY NOT NULL, user_id TEXT NOT NULL, ip TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT (datetime('now')), FOREIGN KEY (user_id) REFERENCES users(id))").execute(pool).await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_ip_whitelist_user_ip ON ip_whitelist(user_id, ip)",
    )
    .execute(pool)
    .await?;
    Ok(())
}
