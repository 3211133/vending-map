use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;
use vending_map_backend::{
    models::image::InsertImageInput,
    repositories::image_repo,
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
        INSERT INTO vending_machines (geom, vending_type, source)
        VALUES (ST_SetSRID(ST_MakePoint(139.7, 35.7), 4326), 'drinks', 'osm')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("insert machine")
}

async fn insert_image(pool: &PgPool, machine_id: Uuid, sha256: &str) -> Uuid {
    let id = Uuid::new_v4();
    image_repo::insert_image(
        pool,
        &InsertImageInput {
            id,
            machine_id,
            file_path: format!("/tmp/test-{id}.jpg"),
            mime_type: "image/jpeg".to_string(),
            file_size: 123,
            sha256: sha256.to_string(),
            uploaded_by: None,
        },
    )
    .await
    .expect("insert image");
    id
}

/// 同一マシン・同一 sha256 の画像が既に存在する場合に検出できることを検証する。
#[tokio::test]
async fn find_image_by_sha256_detects_duplicate() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_id = insert_machine(&pool).await;
    let sha = format!("dup-{}", Uuid::new_v4());
    let image_id = insert_image(&pool, machine_id, &sha).await;

    let found = image_repo::find_image_by_sha256(&pool, machine_id, &sha)
        .await
        .expect("query duplicate");
    assert_eq!(found.map(|img| img.id), Some(image_id));
}

/// sha256 が異なる場合と、別マシンの同一 sha256 は重複として扱わない。
#[tokio::test]
async fn find_image_by_sha256_scopes_to_machine_and_hash() {
    let Some(pool) = test_pool().await else {
        return;
    };

    let machine_a = insert_machine(&pool).await;
    let machine_b = insert_machine(&pool).await;
    let sha = format!("scope-{}", Uuid::new_v4());
    insert_image(&pool, machine_a, &sha).await;

    let other_hash = image_repo::find_image_by_sha256(&pool, machine_a, "no-such-hash")
        .await
        .expect("query other hash");
    assert!(other_hash.is_none(), "異なる sha256 は重複ではない");

    let other_machine = image_repo::find_image_by_sha256(&pool, machine_b, &sha)
        .await
        .expect("query other machine");
    assert!(other_machine.is_none(), "別マシンの同一画像は重複ではない");
}
