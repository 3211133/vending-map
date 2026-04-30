pub mod config;
pub mod error;
pub mod handlers;
pub mod models;
pub mod repositories;
pub mod services;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: config::AppConfig,
}
