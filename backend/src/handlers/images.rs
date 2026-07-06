use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use sha2::{Digest, Sha256};
use tokio::fs;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::image::{image_to_response, ImageResponse, InsertImageInput, InsertItemInput, UploadResponse},
    repositories::{image_repo, vending_machine_repo},
    services::image_analysis,
    AppState,
};

const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10 MB

pub async fn upload_image(
    State(state): State<AppState>,
    Path(machine_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadResponse>), AppError> {
    // Verify the machine exists
    vending_machine_repo::get_by_id(&state.pool, machine_id).await?;

    // Extract image field from multipart
    let field = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("Invalid multipart form"))?
        .ok_or_else(|| AppError::bad_request("No image field in request"))?;

    let mime_type = field
        .content_type()
        .filter(|ct| ct.starts_with("image/"))
        .map(str::to_string)
        .ok_or_else(|| AppError::bad_request("File must be an image (image/* content type)"))?;

    let bytes = field
        .bytes()
        .await
        .map_err(|_| AppError::bad_request("Failed to read image data"))?;

    if bytes.is_empty() {
        return Err(AppError::bad_request("Empty image"));
    }
    if bytes.len() > MAX_FILE_SIZE {
        return Err(AppError::bad_request("Image exceeds 10 MB limit"));
    }
    // MAX_FILE_SIZE (10 MB) チェック済みだが、上限を i32::MAX 超に拡大しても
    // silent truncation しないよう明示的に変換する
    let file_size = i32::try_from(bytes.len())
        .map_err(|_| AppError::bad_request("Image size exceeds supported range"))?;

    // SHA-256 for deduplication detection
    let sha256 = hex::encode(Sha256::digest(&bytes));

    // 同一マシンへの同一画像の再アップロードは既存レコードを返し、再解析しない
    if let Some(existing) =
        image_repo::find_image_by_sha256(&state.pool, machine_id, &sha256).await?
    {
        let items = image_repo::list_items_by_image(&state.pool, existing.id).await?;
        return Ok((
            StatusCode::OK,
            Json(UploadResponse {
                image: image_to_response(existing, items),
            }),
        ));
    }

    let ext = mime_to_ext(&mime_type);
    let image_id = Uuid::new_v4();

    // Persist to filesystem
    let dir = format!("{}/{}", state.config.upload_dir, machine_id);
    let file_path = format!("{}/{}.{}", dir, image_id, ext);
    fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::database(format!("Failed to create upload dir: {e}")))?;
    fs::write(&file_path, &bytes)
        .await
        .map_err(|e| AppError::database(format!("Failed to write image file: {e}")))?;

    // Insert image record (analyzed=false initially)
    let insert_result = image_repo::insert_image(
        &state.pool,
        &InsertImageInput {
            id: image_id,
            machine_id,
            file_path: file_path.clone(),
            mime_type,
            file_size,
            sha256,
            uploaded_by: None,
        },
    )
    .await;
    if insert_result.is_err() {
        // DB 登録に失敗した画像ファイルを残すとディスクに孤児が溜まる
        let _ = fs::remove_file(&file_path).await;
    }
    let _image = insert_result?;

    // Run OCR/color analysis
    let script_path = std::env::var("ANALYZE_SCRIPT")
        .unwrap_or_else(|_| "/app/scripts/analyze_image.py".to_string());

    let (items, status) = match image_analysis::analyze_vending_image(&bytes, &script_path).await {
        Ok(slots) => {
            let inputs: Vec<InsertItemInput> = slots
                .into_iter()
                .filter(|s| s.price_yen.is_some() || s.temperature != "unknown")
                .map(|s| InsertItemInput {
                    image_id,
                    machine_id,
                    name: s.name,
                    price_yen: s.price_yen,
                    temperature: s.temperature,
                })
                .collect();
            let inserted = image_repo::insert_items(&state.pool, &inputs).await?;
            image_repo::mark_analyzed(&state.pool, image_id, None).await?;
            (inserted, StatusCode::CREATED)
        }
        Err(e) => {
            tracing::warn!("Image analysis failed for {image_id}: {e}");
            let msg = e.to_string();
            image_repo::mark_analyzed(&state.pool, image_id, Some(&msg)).await?;
            (vec![], StatusCode::ACCEPTED)
        }
    };

    // Re-fetch image to get updated analyzed flag
    let image = image_repo::get_image_by_id(&state.pool, image_id).await?;
    Ok((status, Json(UploadResponse {
        image: image_to_response(image, items),
    })))
}

pub async fn list_images(
    State(state): State<AppState>,
    Path(machine_id): Path<Uuid>,
) -> Result<Json<Vec<ImageResponse>>, AppError> {
    let images = image_repo::list_images_by_machine(&state.pool, machine_id).await?;
    let mut responses = Vec::with_capacity(images.len());
    for img in images {
        let items = image_repo::list_items_by_image(&state.pool, img.id).await?;
        responses.push(image_to_response(img, items));
    }
    Ok(Json(responses))
}

pub async fn get_image_binary(
    State(state): State<AppState>,
    Path(image_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let image = image_repo::get_image_by_id(&state.pool, image_id).await?;
    let bytes = fs::read(&image.file_path)
        .await
        .map_err(|_| AppError::not_found("Image file not found"))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, image.mime_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn list_items(
    State(state): State<AppState>,
    Path(image_id): Path<Uuid>,
) -> Result<Json<Vec<crate::models::image::ItemResponse>>, AppError> {
    image_repo::get_image_by_id(&state.pool, image_id).await?;
    let items = image_repo::list_items_by_image(&state.pool, image_id).await?;
    Ok(Json(
        items.into_iter().map(crate::models::image::item_to_response).collect(),
    ))
}

fn mime_to_ext(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "jpg",
    }
}
