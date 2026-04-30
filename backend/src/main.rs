use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use vending_map_backend::{
    config::AppConfig,
    error::AppError,
    handlers,
    AppState,
};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    dotenvy::dotenv().ok();

    let config = AppConfig::from_env()?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(config.rust_log.clone()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    // Ensure upload directory exists
    tokio::fs::create_dir_all(&config.upload_dir)
        .await
        .map_err(|e| AppError::database(format!("Failed to create upload dir: {e}")))?;

    let state = AppState {
        pool,
        config: config.clone(),
    };

    let app = Router::new()
        .route("/health", get(handlers::health::health))
        .route(
            "/api/vending-machines",
            get(handlers::vending_machine::list_vending_machines)
                .post(handlers::vending_machine::create_vending_machine),
        )
        .route(
            "/api/vending-machines/:id",
            get(handlers::vending_machine::get_vending_machine)
                .patch(handlers::vending_machine::update_vending_machine)
                .delete(handlers::vending_machine::delete_vending_machine),
        )
        .route(
            "/api/vending-machines/:id/votes",
            post(handlers::votes::create_vote),
        )
        .route(
            "/api/vending-machines/:id/images",
            get(handlers::images::list_images)
                .post(handlers::images::upload_image)
                .layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/api/images/:image_id", get(handlers::images::get_image_binary))
        .route("/api/images/:image_id/items", get(handlers::images::list_items))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.socket_addr()?)
        .await
        .map_err(|e| AppError::database(e.to_string()))?;
    tracing::info!(
        "Listening on {}",
        listener
            .local_addr()
            .map_err(|e| AppError::database(e.to_string()))?
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
        .await
        .map_err(|e| AppError::database(e.to_string()))
}
