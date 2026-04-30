use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateVoteRequest {
    pub vote_type: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct VoteSummary {
    pub exists_count: i64,
    pub removed_count: i64,
    pub trust_score: i16,
    pub status: String,
    pub last_confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct VoteResponse {
    pub machine_id: Uuid,
    pub vote_type: String,
    pub summary: VoteSummary,
}

#[derive(Debug, Clone)]
pub struct CreateVoteInput {
    pub machine_id: Uuid,
    pub voter_fingerprint: String,
    pub ip_prefix: String,
    pub vote_type: String,
}
