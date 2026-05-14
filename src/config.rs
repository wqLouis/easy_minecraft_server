#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:./data/app.db?mode=rwc".to_string());
        Self { database_url }
    }
}
