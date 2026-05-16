mod auth; mod cli; mod cmd; mod db; mod errors; mod ip_ban; mod middleware; mod models; mod serve; mod settings;
use clap::Parser;
use crate::cli::Cli;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./data/app.db?mode=rwc".to_string());
    let pool = db::init_pool(&database_url).await.unwrap_or_else(|e| { eprintln!("Failed to connect to database: {e}"); std::process::exit(1); });
    if let Err(e) = cli::dispatch(Cli::parse(), pool, database_url).await { eprintln!("Error: {e}"); std::process::exit(1); }
}
