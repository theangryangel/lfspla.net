//! Shared file identities and proxied delivery.
use crate::api::{ApiError, ApiState, ErrorResponse};
use crate::models::attachment;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};
use object_store::{ObjectStoreExt, path::Path as ObjectPath};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, prelude::Uuid};
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

/// A stored file. Anyone with its UUID can download it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct Attachment {
    /// Opaque public UUID, independent of the storage key.
    #[schema(value_type = String, format = "uuid")]
    pub(crate) id: Uuid,
    /// Download address; may be relative to the API origin.
    pub(crate) url: String,
    #[schema(required)]
    pub(crate) original_filename: Option<String>,
}
impl Attachment {
    pub(crate) fn new(id: Uuid, original_filename: Option<String>) -> Self {
        Self {
            id,
            original_filename,
            url: format!("/api/v1/attachments/{id}"),
        }
    }
}

/// Loads file identities in one query for a response collection.
pub(crate) async fn load(
    database: &impl ConnectionTrait,
    ids: impl IntoIterator<Item = i64>,
) -> Result<HashMap<i64, Attachment>, ApiError> {
    let ids: Vec<_> = ids.into_iter().collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    attachment::Entity::find()
        .filter(attachment::Column::Id.is_in(ids))
        .all(database)
        .await
        .map_err(ApiError::database)
        .map(|rows| {
            rows.into_iter()
                .map(|row| {
                    (
                        row.id,
                        Attachment::new(row.public_id, row.original_filename),
                    )
                })
                .collect()
        })
}

pub(crate) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(download))
}

/// Serves a file by its public UUID.
#[utoipa::path(get, path = "/api/v1/attachments/{attachment}", operation_id = "download_attachment", tag = "attachments",
    params(("attachment" = String, Path, format = "uuid", description = "Public attachment UUID")),
    responses((status = 200, description = "Stored file streamed with 24-hour browser caching"), (status = 404, description = "Attachment unavailable", body = ErrorResponse)))]
pub(crate) async fn download(
    Path(id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Response, ApiError> {
    let attachment = attachment::Entity::find()
        .filter(attachment::Column::PublicId.eq(id))
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(not_found)?;
    let disposition = crate::storage::download_disposition(&attachment);
    let path = ObjectPath::from(attachment.object_key.as_str());
    let file = state
        .object_store
        .get(&path)
        .await
        .map_err(storage_read_error)?;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &attachment.media_type)
        .header(header::CONTENT_LENGTH, file.meta.size)
        .header(header::CACHE_CONTROL, "private, max-age=86400")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from_stream(file.into_stream()))
        .map_err(|error| ApiError::internal(&error, "attachment response"))
}
fn storage_read_error(error: object_store::Error) -> ApiError {
    match error {
        object_store::Error::NotFound { .. } => not_found(),
        error => ApiError::object_store_read(error),
    }
}
fn not_found() -> ApiError {
    ApiError::not_found("attachment_not_found", "Attachment")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::TransactionTrait;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn request(id: Uuid, state: ApiState) -> Response {
        // Include the API's default no-store middleware to check attachment overrides.
        crate::api::router(state.hotlaps.max_spr_upload_bytes())
            .with_state(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/v1/attachments/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn downloads_stream_bytes_with_filenames_and_browser_caching(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        let store = Arc::new(object_store::memory::InMemory::new());
        let transaction = database.begin().await?;
        let first = attachment::Model::store(
            &transaction,
            &*store,
            b"same bytes".to_vec(),
            "spr",
            "application/octet-stream",
            Some("first lap.spr"),
        )
        .await?;
        let second = attachment::Model::store(
            &transaction,
            &*store,
            b"same bytes".to_vec(),
            "spr",
            "application/octet-stream",
            Some("second lap.spr"),
        )
        .await?;
        let raf = attachment::Model::store(
            &transaction,
            &*store,
            b"telemetry".to_vec(),
            "raf",
            "application/octet-stream",
            Some("actual-LFS-output.RAF"),
        )
        .await?;
        let image = attachment::Model::store(
            &transaction,
            &*store,
            b"image".to_vec(),
            "png",
            "image/png",
            None,
        )
        .await?;
        let unnamed = attachment::Model::store(
            &transaction,
            &*store,
            b"archive".to_vec(),
            "zip",
            "application/zip",
            None,
        )
        .await?;
        transaction.commit().await?;
        assert_ne!(first.public_id, second.public_id);
        assert_eq!(first.object_key, second.object_key);
        let state = ApiState {
            site_notice: None,
            database,
            oauth: None,
            object_store: store.clone(),
            hotlaps: Default::default(),
        };
        for (file, bytes) in [
            (&first, b"same bytes".as_slice()),
            (&second, b"same bytes".as_slice()),
            (&raf, b"telemetry".as_slice()),
            (&image, b"image".as_slice()),
            (&unnamed, b"archive".as_slice()),
        ] {
            // No parent reference or credentials are required: UUIDs are public links.
            let response = request(file.public_id, state.clone()).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "private, max-age=86400"
            );
            assert_eq!(response.headers()[header::CONTENT_TYPE], file.media_type);
            assert_eq!(
                response.headers()[header::CONTENT_LENGTH],
                bytes.len().to_string()
            );
            assert_eq!(
                response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                "nosniff"
            );
            assert!(!response.headers().contains_key(header::LOCATION));
            assert_eq!(
                response.headers()[header::CONTENT_DISPOSITION],
                crate::storage::download_disposition(file)
            );
            assert_eq!(
                axum::body::to_bytes(response.into_body(), 100).await?,
                bytes
            );
        }
        assert!(
            crate::storage::download_disposition(&unnamed)
                .contains(&format!("{}.zip", unnamed.public_id))
        );
        assert!(crate::storage::download_disposition(&image).starts_with("inline;"));
        assert_eq!(
            download(Path(Uuid::from_u128(99)), State(state.clone()))
                .await
                .unwrap_err()
                .code,
            "attachment_not_found"
        );
        let response = request(Uuid::from_u128(99), state.clone()).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        store.delete(&ObjectPath::from(raf.object_key)).await?;
        assert_eq!(
            download(Path(raf.public_id), State(state.clone()))
                .await
                .unwrap_err()
                .code,
            "attachment_not_found"
        );
        store
            .delete(&ObjectPath::from(image.object_key.as_str()))
            .await?;
        let response = request(image.public_id, state.clone()).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        Ok(())
    }
}
