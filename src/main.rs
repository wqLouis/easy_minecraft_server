mod auth;
mod blacklist;
mod cli;
mod config;
mod db;
mod errors;
mod ip_ban;
mod middleware;
mod models;
mod serve;
mod settings;

use clap::Parser;

use crate::cli::Cli;
use crate::config::AppConfig;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Always init DB config (needed by both CLI commands and server)
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
