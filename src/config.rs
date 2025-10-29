use std::env;

use anyhow::Context;
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub audiobook_location: String,
    pub jwt_secret: anyhow::Result<String>,
    pub self_hosted: bool,
    pub jwt_loc: String,
    pub relay_uri: String,
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
            audiobook_location: env::var("AUDIOBOOKS_LOCATION")
                .unwrap_or_else(|_| "data".to_string()),
            jwt_secret: env::var("JWT_SECRET").with_context(|| "Please set JWT SECRET"),
            self_hosted: env::var("SELF_HOSTED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(true),
            jwt_loc: env::var("JWT_LOC").unwrap_or("creds/jwt.key".to_string()),
            relay_uri: env::var("RELAY_URI").unwrap_or("http://localhost:9000/api".to_string()),
        })
    }
}
