use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;
use vending_map_backend::{
    error::AppError, models::vote::CreateVoteInput, repositories::vote_repo,
};

async fn test_pool() -> Option<PgPool> {
    let Ok(db_url) = std::env::var("DATABASE_URL") else {
        eprintln!("Skipping: DATABASE_URL not set");
        return None;
    };

    Some(
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("connect to test database"),
    )
}

async fn insert_machine(pool: &PgPool, status: Option<&str>) -> Uuid {
    let id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO vending_machines (geom, vending_type, source, status)
        VALUES (
            ST_SetSRID(ST_MakePoint(139.7, 35.7), 4326),
            'drinks',
            'osm',
            COALESCE($1, 'unverified')
        )
        RETURNING id
        "#,
    )
    .bind(status)
    .fetch_one(pool)
    .await
    .expect("insert machine");

    id
}

async fn insert_vote(
    pool: &PgPool,
    machine_id: Uuid,
    fingerprint: &str,
    ip_prefix: &str,
    vote_type: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO machine_votes (machine_id, voter_fingerprint, ip_prefix, vote_type)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(machine_id)
    .bind(fingerprint)
    .bind(ip_prefix)
    .bind(vote_type)
    .execute(pool)
    .await
    .expect("insert vote");
}

/// Marks an unverified machine as active after it receives an exists vote.
/// Verifies the trust score increment and exists vote count in the summary.
#[tokio::test]
async fn test_unverified_to_active() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool, Some("unverified")).await;
    insert_vote(&pool, machine_id, "fp-exists-1", "10.0.0.*", "exists").await;

    let summary = vote_repo::refresh_machine_status(&pool, machine_id)
        .await
        .expect("refresh status");

    assert_eq!(summary.status, "active");
    assert_eq!(summary.trust_score, 52);
    assert_eq!(summary.exists_count, 1);
}

/// Demotes an active machine to suspected_removed after repeated removed votes.
/// Confirms the resulting trust score matches the removed-vote penalty.
#[tokio::test]
async fn test_active_to_suspected_removed() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool, Some("active")).await;
    insert_vote(&pool, machine_id, "fp-removed-1", "10.0.1.*", "removed").await;
    insert_vote(&pool, machine_id, "fp-removed-2", "10.0.1.*", "removed").await;
    insert_vote(&pool, machine_id, "fp-removed-3", "10.0.1.*", "removed").await;

    let summary = vote_repo::refresh_machine_status(&pool, machine_id)
        .await
        .expect("refresh status");

    assert_eq!(summary.status, "suspected_removed");
    assert_eq!(summary.trust_score, 41);
}

/// Finalizes a suspected_removed machine as removed when enough recent removed votes exist.
/// Ensures stale exists votes do not prevent the removal transition.
#[tokio::test]
async fn test_suspected_removed_to_removed() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool, Some("suspected_removed")).await;

    insert_vote(&pool, machine_id, "fp-old-exists", "10.0.2.*", "exists").await;
    sqlx::query(
        r#"
        UPDATE machine_votes
        SET created_at = NOW() - INTERVAL '31 days'
        WHERE machine_id = $1 AND voter_fingerprint = 'fp-old-exists'
        "#,
    )
    .bind(machine_id)
    .execute(&pool)
    .await
    .expect("age exists vote");

    insert_vote(&pool, machine_id, "fp-removed-1", "10.0.2.*", "removed").await;
    insert_vote(&pool, machine_id, "fp-removed-2", "10.0.2.*", "removed").await;
    insert_vote(&pool, machine_id, "fp-removed-3", "10.0.2.*", "removed").await;
    insert_vote(&pool, machine_id, "fp-removed-4", "10.0.2.*", "removed").await;
    insert_vote(&pool, machine_id, "fp-removed-5", "10.0.2.*", "removed").await;

    let summary = vote_repo::refresh_machine_status(&pool, machine_id)
        .await
        .expect("refresh status");

    assert_eq!(summary.status, "removed");
}

/// Revives a removed machine back to active after enough exists votes are recorded.
#[tokio::test]
async fn test_removed_to_active_revival() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool, Some("removed")).await;
    insert_vote(&pool, machine_id, "fp-exists-1", "10.0.3.*", "exists").await;
    insert_vote(&pool, machine_id, "fp-exists-2", "10.0.3.*", "exists").await;

    let summary = vote_repo::refresh_machine_status(&pool, machine_id)
        .await
        .expect("refresh status");

    assert_eq!(summary.status, "active");
}

/// Clamps the trust score at zero even if removed votes would push it negative.
#[tokio::test]
async fn test_trust_score_floor() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool, None).await;
    for i in 0..20 {
        insert_vote(
            &pool,
            machine_id,
            &format!("fp-floor-{i}"),
            "10.0.4.*",
            "removed",
        )
        .await;
    }

    let summary = vote_repo::refresh_machine_status(&pool, machine_id)
        .await
        .expect("refresh status");

    assert_eq!(summary.trust_score, 0);
}

/// Clamps the trust score at one hundred even if exists votes would push it higher.
#[tokio::test]
async fn test_trust_score_ceiling() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool, None).await;
    for i in 0..30 {
        insert_vote(
            &pool,
            machine_id,
            &format!("fp-ceiling-{i}"),
            "10.0.5.*",
            "exists",
        )
        .await;
    }

    let summary = vote_repo::refresh_machine_status(&pool, machine_id)
        .await
        .expect("refresh status");

    assert_eq!(summary.trust_score, 100);
}

/// Rejects a second vote from the same fingerprint and surfaces the ALREADY_VOTED conflict code.
#[tokio::test]
async fn test_duplicate_vote_returns_already_voted() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool, None).await;
    insert_vote(&pool, machine_id, "fp-dup", "10.0.6.*", "exists").await;

    let err = vote_repo::create_vote(
        &pool,
        &CreateVoteInput {
            machine_id,
            voter_fingerprint: "fp-dup".to_string(),
            ip_prefix: "10.0.6.*".to_string(),
            vote_type: "exists".to_string(),
        },
    )
    .await
    .expect_err("duplicate vote should fail");

    match err {
        AppError::Conflict { code, .. } => assert_eq!(code, "ALREADY_VOTED"),
        other => panic!("expected conflict error, got {other:?}"),
    }
}

/// Counts recent votes by IP prefix so rate limiting can trigger after repeated submissions.
#[tokio::test]
async fn test_rate_limit_detection() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool, None).await;
    for i in 0..5 {
        insert_vote(
            &pool,
            machine_id,
            &format!("fp-rate-{i}"),
            "10.0.7.*",
            "exists",
        )
        .await;
    }

    let count = vote_repo::count_recent_votes_by_prefix(&pool, machine_id, "10.0.7.*")
        .await
        .expect("count recent votes");

    assert!(count >= 5, "expected at least 5 recent votes, got {count}");
}
