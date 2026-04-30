use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    error::AppError,
    handlers::{to_feature_collection, to_geojson_feature},
    models::vending_machine::{
        CreateVendingMachineInput, CreateVendingMachineRequest, ListVendingMachinesParams,
        UpdateVendingMachineInput, UpdateVendingMachineRequest,
    },
    repositories::vending_machine_repo,
    AppState,
};

pub async fn list_vending_machines(
    State(state): State<AppState>,
    Query(params): Query<ListVendingMachinesParams>,
) -> Result<Json<crate::models::vending_machine::FeatureCollection>, AppError> {
    validate_bbox(params.south, params.west, params.north, params.east)?;
    let machines = vending_machine_repo::list_in_bbox(&state.pool, &params).await?;
    Ok(Json(to_feature_collection(machines)))
}

pub async fn create_vending_machine(
    State(state): State<AppState>,
    Json(payload): Json<CreateVendingMachineRequest>,
) -> Result<(StatusCode, Json<crate::models::vending_machine::Feature>), AppError> {
    validate_coordinate(payload.latitude, payload.longitude)?;
    let machine = vending_machine_repo::create(
        &state.pool,
        &CreateVendingMachineInput {
            latitude: payload.latitude,
            longitude: payload.longitude,
            name: payload.name,
            vending_type: payload.vending_type.unwrap_or_else(|| "drinks".to_string()),
            operator: payload.operator,
            opening_hours: payload.opening_hours,
            source: "user".to_string(),
            osm_id: None,
            osm_tags: None,
        },
    )
    .await?;

    Ok((StatusCode::CREATED, Json(to_geojson_feature(machine))))
}

pub async fn get_vending_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::models::vending_machine::Feature>, AppError> {
    let machine = vending_machine_repo::get_by_id(&state.pool, id).await?;
    Ok(Json(to_geojson_feature(machine)))
}

pub async fn update_vending_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateVendingMachineRequest>,
) -> Result<Json<crate::models::vending_machine::Feature>, AppError> {
    if let Some(status) = payload.status.as_deref() {
        validate_status(status)?;
    }
    let machine = vending_machine_repo::update(
        &state.pool,
        &UpdateVendingMachineInput {
            id,
            name: payload.name,
            vending_type: payload.vending_type,
            operator: payload.operator,
            opening_hours: payload.opening_hours,
            is_active: payload.is_active,
            status: payload.status,
        },
    )
    .await?;

    Ok(Json(to_geojson_feature(machine)))
}

pub async fn delete_vending_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    vending_machine_repo::deactivate(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn validate_bbox(south: f64, west: f64, north: f64, east: f64) -> Result<(), AppError> {
    if south >= north || west >= east {
        return Err(AppError::bad_request("Invalid bbox ordering"));
    }
    validate_coordinate(south, west)?;
    validate_coordinate(north, east)?;
    Ok(())
}

fn validate_coordinate(latitude: f64, longitude: f64) -> Result<(), AppError> {
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err(AppError::bad_request("Coordinates out of range"));
    }
    Ok(())
}

fn validate_status(status: &str) -> Result<(), AppError> {
    match status {
        "active" | "suspected_removed" | "removed" | "unverified" => Ok(()),
        _ => Err(AppError::bad_request("Invalid status")),
    }
}
