use chrono::{Duration, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::vote::{CreateVoteInput, VoteSummary},
};

pub async fn count_recent_votes_by_prefix(
    pool: &PgPool,
    machine_id: Uuid,
    ip_prefix: &str,
) -> Result<i64, AppError> {
    let since = Utc::now() - Duration::hours(24);
    let row = sqlx::query(
        r#"
        SELECT COUNT(*)::BIGINT AS count
        FROM machine_votes
        WHERE machine_id = $1
          AND ip_prefix = $2
          AND created_at >= $3
        "#,
    )
    .bind(machine_id)
    .bind(ip_prefix)
    .bind(since)
    .fetch_one(pool)
    .await?;

    Ok(row.get::<i64, _>("count"))
}

pub async fn create_vote(pool: &PgPool, input: &CreateVoteInput) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO machine_votes (machine_id, voter_fingerprint, ip_prefix, vote_type)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(input.machine_id)
    .bind(&input.voter_fingerprint)
    .bind(&input.ip_prefix)
    .bind(&input.vote_type)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn refresh_machine_status(
    pool: &PgPool,
    machine_id: Uuid,
) -> Result<VoteSummary, AppError> {
    let row = sqlx::query(
        r#"
        WITH summary AS (
            SELECT
                COUNT(*) FILTER (WHERE vote_type = 'exists')::BIGINT AS exists_count,
                COUNT(*) FILTER (WHERE vote_type = 'removed')::BIGINT AS removed_count,
                MAX(created_at) FILTER (WHERE vote_type = 'exists') AS last_exists_vote
            FROM machine_votes
            WHERE machine_id = $1
        ),
        updated AS (
            UPDATE vending_machines vm
            SET
                trust_score = GREATEST(0, LEAST(100, 50 + (summary.exists_count * 2) - (summary.removed_count * 3)))::SMALLINT,
                status = CASE
                    WHEN vm.status = 'removed' AND summary.exists_count >= 2
                        THEN 'active'
                    WHEN summary.removed_count >= 5
                         AND (
                             summary.last_exists_vote IS NULL
                             OR summary.last_exists_vote < NOW() - INTERVAL '30 days'
                         )
                        THEN 'removed'
                    WHEN summary.removed_count >= 3
                         AND summary.removed_count > summary.exists_count * 0.5
                        THEN 'suspected_removed'
                    WHEN summary.exists_count >= 1
                        THEN 'active'
                    ELSE 'unverified'
                END,
                confirmed_at = CASE
                    WHEN summary.exists_count >= 1 THEN summary.last_exists_vote
                    ELSE vm.confirmed_at
                END,
                last_verified = CASE
                    WHEN summary.last_exists_vote IS NOT NULL THEN summary.last_exists_vote::DATE
                    ELSE vm.last_verified
                END,
                updated_at = NOW()
            FROM summary
            WHERE vm.id = $1
            RETURNING
                summary.exists_count,
                summary.removed_count,
                vm.trust_score,
                vm.status,
                vm.confirmed_at
        )
        SELECT
            exists_count,
            removed_count,
            trust_score,
            status,
            confirmed_at AS last_confirmed_at
        FROM updated
        "#,
    )
    .bind(machine_id)
    .fetch_one(pool)
    .await?;

    Ok(VoteSummary {
        exists_count: row.get("exists_count"),
        removed_count: row.get("removed_count"),
        trust_score: row.get("trust_score"),
        status: row.get("status"),
        last_confirmed_at: row.get("last_confirmed_at"),
    })
}
