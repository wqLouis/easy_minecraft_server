mod auth;
mod cli;
mod db;
mod errors;
mod ip_ban;
mod middleware;
mod models;
mod serve;
mod settings;

use clap::Parser;

use crate::cli::Cli;

#[derive(Debug, Clone)]
struct AppConfig {
    database_url: String,
}

impl AppConfig {
    fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:./data/app.db?mode=rwc".to_string());
        Self { database_url }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    dotenvy::dotenv().ok();
    let config = AppConfig::from_env();

    let pool = match db::init_pool(&config.database_url).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = cli::dispatch(cli, pool).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
