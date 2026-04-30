use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::vending_machine::{
        CreateVendingMachineInput, ListVendingMachinesParams, UpdateVendingMachineInput,
        VendingMachine,
    },
};

pub async fn list_in_bbox(
    pool: &PgPool,
    params: &ListVendingMachinesParams,
) -> Result<Vec<VendingMachine>, AppError> {
    let limit = params.limit.unwrap_or(500).clamp(1, 1000);

    let machines = sqlx::query_as::<_, VendingMachine>(
        r#"
        SELECT
            vm.id,
            vm.osm_id,
            ST_Y(vm.geom) AS latitude,
            ST_X(vm.geom) AS longitude,
            vm.name,
            vm.vending_type,
            vm.operator,
            vm.opening_hours,
            vm.source,
            vm.osm_tags,
            vm.is_active,
            vm.trust_score,
            vm.status,
            vm.confirmed_at,
            vm.last_verified,
            COALESCE(v.exists_count, 0) AS exists_count,
            COALESCE(v.removed_count, 0) AS removed_count,
            vm.created_at,
            vm.updated_at
        FROM vending_machines vm
        LEFT JOIN (
            SELECT
                machine_id,
                COUNT(*) FILTER (WHERE vote_type = 'exists')::BIGINT AS exists_count,
                COUNT(*) FILTER (WHERE vote_type = 'removed')::BIGINT AS removed_count
            FROM machine_votes
            GROUP BY machine_id
        ) v ON v.machine_id = vm.id
        WHERE vm.is_active = TRUE
          AND ST_Intersects(
                vm.geom,
                ST_MakeEnvelope($1, $2, $3, $4, 4326)
              )
          AND ($5::TEXT IS NULL OR vm.vending_type = $5)
        ORDER BY vm.updated_at DESC
        LIMIT $6
        "#,
    )
    .bind(params.west)
    .bind(params.south)
    .bind(params.east)
    .bind(params.north)
    .bind(params.vending_type.as_deref())
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(machines)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<VendingMachine, AppError> {
    let machine = sqlx::query_as::<_, VendingMachine>(
        r#"
        SELECT
            vm.id,
            vm.osm_id,
            ST_Y(vm.geom) AS latitude,
            ST_X(vm.geom) AS longitude,
            vm.name,
            vm.vending_type,
            vm.operator,
            vm.opening_hours,
            vm.source,
            vm.osm_tags,
            vm.is_active,
            vm.trust_score,
            vm.status,
            vm.confirmed_at,
            vm.last_verified,
            COALESCE(v.exists_count, 0) AS exists_count,
            COALESCE(v.removed_count, 0) AS removed_count,
            vm.created_at,
            vm.updated_at
        FROM vending_machines vm
        LEFT JOIN (
            SELECT
                machine_id,
                COUNT(*) FILTER (WHERE vote_type = 'exists')::BIGINT AS exists_count,
                COUNT(*) FILTER (WHERE vote_type = 'removed')::BIGINT AS removed_count
            FROM machine_votes
            GROUP BY machine_id
        ) v ON v.machine_id = vm.id
        WHERE vm.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(machine)
}

pub async fn create(
    pool: &PgPool,
    input: &CreateVendingMachineInput,
) -> Result<VendingMachine, AppError> {
    let machine = sqlx::query_as::<_, VendingMachine>(
        r#"
        INSERT INTO vending_machines (
            osm_id,
            geom,
            name,
            vending_type,
            operator,
            opening_hours,
            source,
            osm_tags
        )
        VALUES (
            $1,
            ST_SetSRID(ST_MakePoint($2, $3), 4326),
            $4,
            $5,
            $6,
            $7,
            $8,
            $9
        )
        RETURNING
            id,
            osm_id,
            ST_Y(geom) AS latitude,
            ST_X(geom) AS longitude,
            name,
            vending_type,
            operator,
            opening_hours,
            source,
            osm_tags,
            is_active,
            trust_score,
            status,
            confirmed_at,
            last_verified,
            0::BIGINT AS exists_count,
            0::BIGINT AS removed_count,
            created_at,
            updated_at
        "#,
    )
    .bind(input.osm_id)
    .bind(input.longitude)
    .bind(input.latitude)
    .bind(input.name.as_deref())
    .bind(&input.vending_type)
    .bind(input.operator.as_deref())
    .bind(input.opening_hours.as_deref())
    .bind(&input.source)
    .bind(input.osm_tags.clone())
    .fetch_one(pool)
    .await?;

    Ok(machine)
}

pub async fn update(
    pool: &PgPool,
    input: &UpdateVendingMachineInput,
) -> Result<VendingMachine, AppError> {
    let machine = sqlx::query_as::<_, VendingMachine>(
        r#"
        UPDATE vending_machines
        SET
            name = COALESCE($2, name),
            vending_type = COALESCE($3, vending_type),
            operator = COALESCE($4, operator),
            opening_hours = COALESCE($5, opening_hours),
            is_active = COALESCE($6, is_active),
            status = COALESCE($7, status),
            updated_at = NOW()
        WHERE id = $1
        RETURNING
            id,
            osm_id,
            ST_Y(geom) AS latitude,
            ST_X(geom) AS longitude,
            name,
            vending_type,
            operator,
            opening_hours,
            source,
            osm_tags,
            is_active,
            trust_score,
            status,
            confirmed_at,
            last_verified,
            0::BIGINT AS exists_count,
            0::BIGINT AS removed_count,
            created_at,
            updated_at
        "#,
    )
    .bind(input.id)
    .bind(input.name.as_deref())
    .bind(input.vending_type.as_deref())
    .bind(input.operator.as_deref())
    .bind(input.opening_hours.as_deref())
    .bind(input.is_active)
    .bind(input.status.as_deref())
    .fetch_one(pool)
    .await?;

    Ok(machine)
}

pub async fn deactivate(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"
        UPDATE vending_machines
        SET is_active = FALSE, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Vending machine not found"));
    }

    Ok(())
}
