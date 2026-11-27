use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Deserialize)]
pub struct ThumbQuery {
    /// "large" for the preview; the list thumbnail otherwise.
    size: Option<String>,
}

/// A published file's thumbnail (JPEG), by its content hash. 202 while it is being drawn, 404
/// when there is nothing to show (`error.code` "no_thumbnail").
pub async fn get(State(s): State<AppState>, user: CurrentUser, Path((p, hash)): Path<(Uuid, String)>, Query(q): Query<ThumbQuery>) -> ApiResult<Response> {
    access::project(&s, &user, p, Level::Read).await?;
    if !mp_core::manifest::is_hash(&hash) {
        return Err(ApiError::not_found("That file"));
    }
    // The file must be in this project (a hash from another project shows nothing).
    let file = sqlx::query(
        "SELECT t.path, t.manifest_hash, t.size FROM tree_entries t JOIN branches b ON b.id = t.branch_id WHERE b.project_id = $1 AND t.file_hash = $2
         UNION ALL SELECT path, manifest_hash, size FROM change_files WHERE project_id = $1 AND file_hash = $2 LIMIT 1",
    )
    .bind(p)
    .bind(&hash)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| ApiError::not_found("That file"))?;
    let large = q.size.as_deref() == Some("large");
    let row = sqlx::query(&format!("SELECT status, width, height, {} AS img FROM thumbnails WHERE file_hash = $1", if large { "large" } else { "small" }))
        .bind(&hash)
        .fetch_optional(&s.db)
        .await?;
    let none = || ApiError::new(StatusCode::NOT_FOUND, "no_thumbnail", "There is no picture of this file.");
    let Some(row) = row else {
        let path: String = file.get("path");
        if crate::thumbs::source_of(&path).is_none() {
            return Err(none());
        }
        crate::thumbs::enqueue(&s.db, p, &[(path, hash.clone(), file.get("manifest_hash"), file.get("size"))]).await;
        return Ok((StatusCode::ACCEPTED, Json(json!({ "status": "pending" }))).into_response());
    };
    match row.get::<String, _>("status").as_str() {
        "ready" => {
            let img: Vec<u8> = row.get::<Option<Vec<u8>>, _>("img").unwrap_or_default();
            Ok((
                [
                    (header::CONTENT_TYPE, "image/jpeg".to_string()),
                    // The address names the content, so it never changes.
                    (header::CACHE_CONTROL, "private, max-age=31536000, immutable".to_string()),
                    (header::HeaderName::from_static("x-image-size"), format!("{}x{}", row.get::<Option<i32>, _>("width").unwrap_or(0), row.get::<Option<i32>, _>("height").unwrap_or(0))),
                ],
                img,
            )
                .into_response())
        }
        "pending" => Ok((StatusCode::ACCEPTED, Json(json!({ "status": "pending" }))).into_response()),
        _ => Err(none()),
    }
}
