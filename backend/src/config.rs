use std::{env, net::SocketAddr};

use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub rust_log: String,
    pub server_host: String,
    pub server_port: u16,
    pub daily_secret: String,
    pub upload_dir: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            database_url: read_var("DATABASE_URL")?,
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .map_err(|_| AppError::bad_request("Invalid SERVER_PORT"))?,
            daily_secret: read_var("DAILY_SECRET")?,
            upload_dir: env::var("UPLOAD_DIR").unwrap_or_else(|_| "/app/uploads".to_string()),
        })
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, AppError> {
        let addr = format!("{}:{}", self.server_host, self.server_port);
        addr.parse()
            .map_err(|_| AppError::bad_request("Invalid server bind address"))
    }
}

fn read_var(key: &str) -> Result<String, AppError> {
    env::var(key).map_err(|_| AppError::bad_request(format!("Missing environment variable: {key}")))
}
