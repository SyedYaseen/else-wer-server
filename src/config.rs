use std::env;

use anyhow::Context;
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub book_files: String,
    pub jwt_secret: anyhow::Result<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, anyhow::Error> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./rustybookshelf.db".to_string()),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap_or(3000),
            book_files: env::var("AUDIOBOOKS_LOCATION").unwrap_or_else(|_| "data".to_string()),
            jwt_secret: env::var("JWT_SECRET").with_context(|| "Please set JWT SECRET"),
        })
    }
}
