use axum::{
    Json,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use keepsake_core::media::{self, MediaFiles, MediaUpload};
use sqlx::SqlitePool;
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::{
    facts_api::{ApiError, ErrorBody},
    profile_context::{Actor, ItemPath},
};

/// Marks a response the cache policy may keep: media bytes never change under an ID.
#[derive(Clone, Copy)]
pub(crate) struct Immutable;

/// A file's raw bytes, as the OpenAPI document describes them.
#[derive(utoipa::ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct FileBytes(#[allow(dead_code)] Vec<u8>);

fn too_large() -> ApiError {
    ApiError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        "Files must be 5 MiB or smaller",
    )
}

#[utoipa::path(post, path = "/api/media",
    request_body(content = inline(FileBytes), content_type = "application/octet-stream",
        description = "The file's bytes, sent with its own Content-Type, such as image/png or audio/mpeg. \
            It must decode as that type: a still JPEG, PNG or WebP up to 4096 pixels a side, \
            or MP3, M4A, AAC, WAV, Ogg, WebM or FLAC up to 5 minutes. At most 5 MiB. \
            Images are stored as AVIF, at most 2048 pixels on the long edge, and clips as \
            AAC in MP4."),
    responses(
        (status = 201, body = MediaUpload), (status = 400, body = ErrorBody),
        (status = 409, body = ErrorBody), (status = 413, body = ErrorBody),
        (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
    ))]
pub async fn upload(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    headers: HeaderMap,
    body: Body,
) -> Result<(StatusCode, Json<MediaUpload>), ApiError> {
    // The declared type is checked against the bytes, never trusted on its own.
    let declared = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Send the file's type in the Content-Type header",
            )
        })?
        .to_owned();
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok()?.parse::<usize>().ok())
        .is_some_and(|length| length > media::MAX_MEDIA_BYTES)
    {
        return Err(too_large());
    }
    let bytes = receive(&pool, body).await?;
    let saved = media::upload(&pool, profile_id, &declared, bytes)
        .await
        .map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(saved)))
}

/// Write the body into `tmp/` as it arrives, refusing it once it passes the size limit.
/// The partial file is removed when this returns, whatever happens.
async fn receive(pool: &SqlitePool, mut body: Body) -> Result<Vec<u8>, ApiError> {
    let storage = |error: std::io::Error| ApiError::internal(error.into());
    let (file, path) = MediaFiles::for_pool(pool)
        .partial_upload()
        .map_err(storage)?
        .into_parts();
    let mut file = tokio::fs::File::from_std(file);
    let mut received = 0usize;
    while let Some(frame) = body.frame().await {
        let frame = frame
            .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "The upload was interrupted"))?;
        let Ok(chunk) = frame.into_data() else {
            continue;
        };
        received += chunk.len();
        if received > media::MAX_MEDIA_BYTES {
            return Err(too_large());
        }
        file.write_all(&chunk).await.map_err(storage)?;
    }
    file.flush().await.map_err(storage)?;
    drop(file);
    let bytes = tokio::fs::read(&path).await.map_err(storage)?;
    drop(path);
    Ok(bytes)
}

#[utoipa::path(get, path = "/api/media/{id}", params(("id" = i64, Path)), responses(
    (status = 200, content_type = "application/octet-stream", body = inline(FileBytes)),
    (status = 206, content_type = "application/octet-stream", body = inline(FileBytes)),
    (status = 404, body = ErrorBody), (status = 416, body = ErrorBody),
    (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn download(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Path(ItemPath { id }): Path<ItemPath>,
    request: Request,
) -> Result<Response, ApiError> {
    let file = media::locate(&pool, profile_id, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Media not found"))?;
    if !tokio::fs::try_exists(&file.path).await.unwrap_or(false) {
        return Err(ApiError::internal(anyhow::anyhow!(
            "media file {} is missing",
            file.path.display()
        )));
    }
    // ServeFile answers byte ranges, which Safari needs to play audio, and streams the file.
    let mut response = match ServeFile::new(&file.path).oneshot(request).await {
        Ok(response) => response.map(Body::new),
        Err(never) => match never {},
    };
    if response.status().is_success() {
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&file.content_type)
                .map_err(|error| ApiError::internal(error.into()))?,
        );
        response.extensions_mut().insert(Immutable);
    }
    Ok(response.into_response())
}
