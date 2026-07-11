use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;
use vending_map_backend::{
    models::vending_machine::UpdateVendingMachineInput, repositories::vending_machine_repo,
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

async fn insert_machine(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO vending_machines (geom, vending_type, source, status)
        VALUES (
            ST_SetSRID(ST_MakePoint(139.7, 35.7), 4326),
            'drinks',
            'osm',
            'active'
        )
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("insert machine")
}

async fn insert_vote(pool: &PgPool, machine_id: Uuid, fingerprint: &str, vote_type: &str) {
    sqlx::query(
        r#"
        INSERT INTO machine_votes (machine_id, voter_fingerprint, ip_prefix, vote_type)
        VALUES ($1, $2, '10.1.0.*', $3)
        "#,
    )
    .bind(machine_id)
    .bind(fingerprint)
    .bind(vote_type)
    .execute(pool)
    .await
    .expect("insert vote");
}

/// T-03 回帰テスト: update() のレスポンスが既存の投票数を保持することを検証。
/// 修正前は RETURNING 句で `0::BIGINT` がハードコードされ、更新直後の
/// API レスポンスから投票数が消えていた。
#[tokio::test]
async fn test_update_preserves_vote_counts() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool).await;
    insert_vote(&pool, machine_id, "fp-update-1", "exists").await;
    insert_vote(&pool, machine_id, "fp-update-2", "exists").await;
    insert_vote(&pool, machine_id, "fp-update-3", "removed").await;

    let updated = vending_machine_repo::update(
        &pool,
        &UpdateVendingMachineInput {
            id: machine_id,
            name: Some("更新後の名前".to_string()),
            vending_type: None,
            operator: None,
            opening_hours: None,
            is_active: None,
            status: None,
        },
    )
    .await
    .expect("update machine");

    assert_eq!(updated.name.as_deref(), Some("更新後の名前"));
    assert_eq!(
        updated.exists_count, 2,
        "update() 直後のレスポンスで exists_count が保持されること (T-03)"
    );
    assert_eq!(
        updated.removed_count, 1,
        "update() 直後のレスポンスで removed_count が保持されること (T-03)"
    );
}
