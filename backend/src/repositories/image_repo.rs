use sqlx::{PgPool, QueryBuilder};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::image::{InsertImageInput, InsertItemInput, MachineImage, MachineItem},
};

pub async fn insert_image(pool: &PgPool, input: &InsertImageInput) -> Result<MachineImage, AppError> {
    let row = sqlx::query_as::<_, MachineImage>(
        r#"
        INSERT INTO machine_images
            (id, machine_id, file_path, mime_type, file_size, sha256, uploaded_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id, machine_id, file_path, mime_type, file_size, sha256,
            analyzed, analyze_error, uploaded_by, created_at
        "#,
    )
    .bind(input.id)
    .bind(input.machine_id)
    .bind(&input.file_path)
    .bind(&input.mime_type)
    .bind(input.file_size)
    .bind(&input.sha256)
    .bind(&input.uploaded_by)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn mark_analyzed(pool: &PgPool, id: Uuid, error: Option<&str>) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE machine_images
        SET analyzed = $2, analyze_error = $3
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(error.is_none())
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_items(pool: &PgPool, items: &[InsertItemInput]) -> Result<Vec<MachineItem>, AppError> {
    if items.is_empty() {
        return Ok(vec![]);
    }

    let mut builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "INSERT INTO machine_items (id, image_id, machine_id, name, price_yen, temperature) ",
    );

    builder.push_values(items, |mut b, item| {
        b.push_bind(Uuid::new_v4())
            .push_bind(item.image_id)
            .push_bind(item.machine_id)
            .push_bind(&item.name)
            .push_bind(item.price_yen)
            .push_bind(&item.temperature);
    });

    builder.push(
        " RETURNING id, image_id, machine_id, name, price_yen, temperature, created_at",
    );

    let rows = builder
        .build_query_as::<MachineItem>()
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn find_image_by_sha256(
    pool: &PgPool,
    machine_id: Uuid,
    sha256: &str,
) -> Result<Option<MachineImage>, AppError> {
    let row = sqlx::query_as::<_, MachineImage>(
        r#"
        SELECT id, machine_id, file_path, mime_type, file_size, sha256,
               analyzed, analyze_error, uploaded_by, created_at
        FROM machine_images
        WHERE machine_id = $1 AND sha256 = $2
        ORDER BY created_at
        LIMIT 1
        "#,
    )
    .bind(machine_id)
    .bind(sha256)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_image_by_id(pool: &PgPool, id: Uuid) -> Result<MachineImage, AppError> {
    let row = sqlx::query_as::<_, MachineImage>(
        r#"
        SELECT id, machine_id, file_path, mime_type, file_size, sha256,
               analyzed, analyze_error, uploaded_by, created_at
        FROM machine_images
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_images_by_machine(pool: &PgPool, machine_id: Uuid) -> Result<Vec<MachineImage>, AppError> {
    let rows = sqlx::query_as::<_, MachineImage>(
        r#"
        SELECT id, machine_id, file_path, mime_type, file_size, sha256,
               analyzed, analyze_error, uploaded_by, created_at
        FROM machine_images
        WHERE machine_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(machine_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_items_by_image(pool: &PgPool, image_id: Uuid) -> Result<Vec<MachineItem>, AppError> {
    let rows = sqlx::query_as::<_, MachineItem>(
        r#"
        SELECT id, image_id, machine_id, name, price_yen, temperature, created_at
        FROM machine_items
        WHERE image_id = $1
        ORDER BY created_at
        "#,
    )
    .bind(image_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
