//! Public track catalogue, independent of era chart selections.

use crate::{
    api::{ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery},
    models::track::{Entity, TrackFilter, TrackOrder},
};
use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::{EntityTrait, PaginatorTrait, QuerySelect};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::eras::tracks::TrackSummary;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TrackQuery {
    /// Case-insensitive track code or name search. Omit to browse all configurations.
    q: Option<String>,
}

pub(crate) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(list_tracks))
}

#[utoipa::path(
    get,
    path = "/api/v1/tracks",
    operation_id = "list_tracks",
    tag = "tracks",
    params(TrackQuery, PaginationQuery),
    responses(
        (status = 200, description = "Canonical track configurations", body = PaginatedResponse<TrackSummary>),
        (status = 400, description = "Invalid pagination", body = ErrorResponse)
    )
)]
pub(crate) async fn list_tracks(
    Query(query): Query<TrackQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<PaginatedResponse<TrackSummary>>, ApiError> {
    let offset = pagination.offset()?;
    let tracks = Entity::find().name_or_id_contains(query.q.as_deref().unwrap_or(""));
    let total_items = tracks
        .clone()
        .count(&state.database)
        .await
        .map_err(ApiError::database)?;
    let tracks = tracks
        .in_catalogue_order()
        .offset(offset)
        .limit(pagination.per_page)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(PaginatedResponse {
        items: tracks.into_iter().map(TrackSummary::from).collect(),
        pagination: pagination.metadata(total_items),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn request(state: ApiState, query: &str) -> axum::response::Response {
        crate::api::router(state.hotlaps.max_spr_upload_bytes())
            .with_state(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/v1/tracks{query}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn body(response: axum::response::Response) -> serde_json::Value {
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn catalogue_search_and_pagination_without_eras(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        crate::models::Track::sync(&database).await?;
        let state = ApiState {
            site_notice: None,
            database,
            oauth: None,
            object_store: Arc::new(object_store::memory::InMemory::new()),
            hotlaps: Default::default(),
        };
        let tracks = insim_core::track::Track::ALL;
        let first = body(request(state.clone(), "?per_page=3").await).await;
        let second = body(request(state.clone(), "?per_page=3&page=2").await).await;
        assert_eq!(first["items"].as_array().unwrap().len(), 3);
        assert_eq!(first["items"][0]["code"], tracks[0].code());
        assert_eq!(second["items"][0]["code"], tracks[3].code());
        assert_eq!(
            second["pagination"],
            serde_json::json!({
                "page": 2, "per_page": 3, "total_items": tracks.len(), "total_pages": tracks.len().div_ceil(3)
            })
        );

        let found = body(request(state.clone(), "?q=%20bl1%20").await).await;
        assert!(
            found["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["code"].as_str().unwrap().contains("BL1"))
        );
        let bl1 = found["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["code"] == "BL1")
            .unwrap();
        assert_eq!(
            bl1["location"],
            serde_json::json!({"code": "BL", "name": "Blackwood"})
        );
        assert_eq!(bl1["reverse"], false);
        assert_eq!(bl1["open_configuration"], false);
        let found = body(request(state.clone(), "?q=bLaCkWoOd").await).await;
        assert!(found["pagination"]["total_items"].as_u64().unwrap() > 0);
        assert!(
            found["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["location"]["code"] == "BL")
        );
        for query in ["?q=ZZ9", "?q=%25", "?q=_", "?page=10000"] {
            let found = body(request(state.clone(), query).await).await;
            assert!(found["items"].as_array().unwrap().is_empty());
        }
        for query in ["?page=0", "?per_page=101", "?page=abc"] {
            assert_eq!(
                request(state.clone(), query).await.status(),
                axum::http::StatusCode::BAD_REQUEST
            );
        }
        Ok(())
    }
}
