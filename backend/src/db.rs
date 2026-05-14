use std::path::Path;

use sqlx::SqlitePool;

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // Ensure the parent directory for the SQLite file exists
    if let Some(path) = database_url
        .strip_prefix("sqlite:")
        .and_then(|s| s.split('?').next())
    {
        if let Some(parent) = Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let pool = SqlitePool::connect(database_url).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id           TEXT PRIMARY KEY NOT NULL,
            email        TEXT NOT NULL UNIQUE,
            api_key_hash TEXT NOT NULL,
            is_sudoer    INTEGER NOT NULL DEFAULT 0,
            created_at   TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
