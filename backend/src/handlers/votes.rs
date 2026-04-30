use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::vote::{CreateVoteInput, CreateVoteRequest, VoteResponse},
    repositories::{vending_machine_repo, vote_repo},
    AppState,
};

pub async fn create_vote(
    State(state): State<AppState>,
    Path(machine_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateVoteRequest>,
) -> Result<(StatusCode, Json<VoteResponse>), AppError> {
    validate_vote_type(&payload.vote_type)?;
    vending_machine_repo::get_by_id(&state.pool, machine_id).await?;

    let client_ip = extract_client_ip(&headers);
    let ip_prefix = ip_prefix(&client_ip);
    let fingerprint = fingerprint(&client_ip, &state.config.daily_secret);

    if vote_repo::count_recent_votes_by_prefix(&state.pool, machine_id, &ip_prefix).await? >= 5 {
        return Err(AppError::rate_limited());
    }

    vote_repo::create_vote(
        &state.pool,
        &CreateVoteInput {
            machine_id,
            voter_fingerprint: fingerprint,
            ip_prefix,
            vote_type: payload.vote_type.clone(),
        },
    )
    .await?;

    let summary = vote_repo::refresh_machine_status(&state.pool, machine_id).await?;
    Ok((
        StatusCode::CREATED,
        Json(VoteResponse {
            machine_id,
            vote_type: payload.vote_type,
            summary,
        }),
    ))
}

fn validate_vote_type(vote_type: &str) -> Result<(), AppError> {
    match vote_type {
        "exists" | "removed" => Ok(()),
        _ => Err(AppError::bad_request("Invalid vote_type")),
    }
}

fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
        })
        .unwrap_or("0.0.0.0")
        .to_string()
}

fn ip_prefix(ip: &str) -> String {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() == 4 {
        return format!("{}.{}.{}.*", parts[0], parts[1], parts[2]);
    }
    ip.to_string()
}

fn fingerprint(ip: &str, secret: &str) -> String {
    let today = Utc::now().date_naive();
    let raw = format!("{ip}|{today}|{secret}");
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}
