use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use keepsake_core::media::{self, MediaUpload};
use sqlx::SqlitePool;

use crate::{
    facts_api::{ApiError, ErrorBody},
    profile_context::{Actor, ItemPath},
};

/// Marks a response the cache policy may keep: media bytes never change under an ID.
#[derive(Clone, Copy)]
pub(crate) struct Immutable;

/// Room for the largest file plus nothing else; uploads are a raw body, not a form.
pub(crate) const UPLOAD_LIMIT: usize = media::MAX_MEDIA_BYTES;

/// A file's raw bytes, as the OpenAPI document describes them.
#[derive(utoipa::ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct FileBytes(#[allow(dead_code)] Vec<u8>);

#[utoipa::path(post, path = "/api/media",
    request_body(content = inline(FileBytes), content_type = "application/octet-stream",
        description = "The file's bytes, sent with its own Content-Type, such as image/png or audio/mpeg. \
            The file must decode as that type: JPEG, PNG, GIF or WebP up to 4096 pixels a side, \
            or MP3, M4A, AAC, WAV, Ogg, WebM or FLAC up to 5 minutes. At most 5 MiB."),
    responses(
        (status = 201, body = MediaUpload), (status = 400, body = ErrorBody),
        (status = 409, body = ErrorBody), (status = 413, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
    ))]
pub async fn upload(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    headers: HeaderMap,
    body: Bytes,
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
        })?;
    let saved = media::upload(&pool, profile_id, declared, body.to_vec())
        .await
        .map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(saved)))
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
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let file = media::read(&pool, profile_id, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Media not found"))?;
    let length = file.data.len();
    // Safari plays audio only from servers that answer byte ranges.
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|value| byte_range(value, length));
    let (status, body, content_range) = match range {
        Some(Range::Satisfiable(start, end)) => (
            StatusCode::PARTIAL_CONTENT,
            file.data[start..=end].to_vec(),
            Some(format!("bytes {start}-{end}/{length}")),
        ),
        Some(Range::Unsatisfiable) => {
            let mut response =
                ApiError::new(StatusCode::RANGE_NOT_SATISFIABLE, "Range not satisfiable")
                    .into_response();
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{length}")).expect("ASCII header"),
            );
            return Ok(response);
        }
        Some(Range::Ignored) | None => (StatusCode::OK, file.data, None),
    };
    let mut response = (status, Body::from(body)).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&file.content_type)
            .map_err(|error| ApiError::internal(error.into()))?,
    );
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Some(content_range) = content_range {
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&content_range).expect("ASCII header"),
        );
    }
    response.extensions_mut().insert(Immutable);
    Ok(response)
}

#[derive(Debug, PartialEq)]
enum Range {
    /// Inclusive byte offsets.
    Satisfiable(usize, usize),
    Unsatisfiable,
    /// Multiple ranges or another unit: the whole file is a valid answer.
    Ignored,
}

fn byte_range(value: &str, length: usize) -> Range {
    let Some(spec) = value.trim().strip_prefix("bytes=") else {
        return Range::Ignored;
    };
    if spec.contains(',') {
        return Range::Ignored;
    }
    let Some((start, end)) = spec.trim().split_once('-') else {
        return Range::Ignored;
    };
    let parse = |value: &str| value.trim().parse::<usize>().ok();
    let (start, end) = match (start.trim().is_empty(), end.trim().is_empty()) {
        // A suffix: the last N bytes.
        (true, false) => match parse(end) {
            Some(0) => return Range::Unsatisfiable,
            Some(suffix) => (length.saturating_sub(suffix), length.saturating_sub(1)),
            None => return Range::Ignored,
        },
        (false, true) => match parse(start) {
            Some(start) => (start, length.saturating_sub(1)),
            None => return Range::Ignored,
        },
        (false, false) => match (parse(start), parse(end)) {
            (Some(start), Some(end)) if start <= end => (start, end.min(length.saturating_sub(1))),
            _ => return Range::Ignored,
        },
        (true, true) => return Range::Ignored,
    };
    if length == 0 || start >= length {
        return Range::Unsatisfiable;
    }
    Range::Satisfiable(start, end)
}

#[cfg(test)]
mod tests {
    use super::{Range, byte_range};

    #[test]
    fn answers_single_byte_ranges_and_ignores_the_rest() {
        for (header, expected) in [
            ("bytes=0-1", Range::Satisfiable(0, 1)),
            ("bytes=2-", Range::Satisfiable(2, 9)),
            ("bytes=-3", Range::Satisfiable(7, 9)),
            ("bytes=-30", Range::Satisfiable(0, 9)),
            ("bytes=5-500", Range::Satisfiable(5, 9)),
            ("bytes=10-", Range::Unsatisfiable),
            ("bytes=-0", Range::Unsatisfiable),
            ("bytes=0-1,4-5", Range::Ignored),
            ("bytes=3-2", Range::Ignored),
            ("bytes=x-", Range::Ignored),
            ("items=0-1", Range::Ignored),
        ] {
            assert_eq!(byte_range(header, 10), expected, "{header}");
        }
    }
}
