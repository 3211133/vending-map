use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::{error::AppError, AppState};

pub async fn health(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    sqlx::query("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map(|_| Json(json!({ "status": "ok" })))
        .map_err(AppError::from)
}
