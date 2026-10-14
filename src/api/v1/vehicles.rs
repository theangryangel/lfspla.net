//! Public vehicle catalogue, independent of era chart selections.

use crate::{
    api::{ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery},
    models::vehicle::{Column, Entity, VehicleFilter, VehicleOrder},
};
use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::eras::vehicles::VehicleSummary;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct VehicleQuery {
    /// Case-insensitive vehicle code or name search. Omit to browse all available vehicles.
    q: Option<String>,
}

pub(crate) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(list_vehicles))
}

#[utoipa::path(
    get,
    path = "/api/v1/vehicles",
    operation_id = "list_vehicles",
    tag = "vehicles",
    params(VehicleQuery, PaginationQuery),
    responses(
        (status = 200, description = "Available standard vehicles and mods", body = PaginatedResponse<VehicleSummary>),
        (status = 400, description = "Invalid pagination", body = ErrorResponse)
    )
)]
pub(crate) async fn list_vehicles(
    Query(query): Query<VehicleQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<PaginatedResponse<VehicleSummary>>, ApiError> {
    let offset = pagination.offset()?;
    let search = query.q.as_deref().unwrap_or("");
    let vehicles = Entity::find()
        .filter(Column::Available.eq(true))
        .name_or_id_contains(search);
    let total_items = vehicles
        .clone()
        .count(&state.database)
        .await
        .map_err(ApiError::database)?;
    let vehicles = vehicles
        .in_catalogue_search_order(search)
        .offset(offset)
        .limit(pagination.per_page)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    let attachments = super::attachments::load(
        &state.database,
        vehicles.iter().filter_map(|v| v.image_attachment_id),
    )
    .await?;
    Ok(Json(PaginatedResponse {
        items: vehicles
            .into_iter()
            .map(|v| VehicleSummary::from_record(v, &attachments))
            .collect(),
        pagination: pagination.metadata(total_items),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn request(state: ApiState, query: &str) -> axum::response::Response {
        crate::api::router(state.hotlaps.max_spr_upload_bytes())
            .with_state(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/v1/vehicles{query}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn body(response: axum::response::Response) -> serde_json::Value {
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
        crate::models::Vehicle::sync_builtin(&database).await?;
        crate::models::Vehicle::refresh(
            &database,
            &[lfsplanet_lfs_api::RemoteVehicleMod {
                id: "ABCDEF".to_owned(),
                name: "Test vehicle".to_owned(),
                version: 1,
                class: None,
                author_username: None,
                work_in_progress: false,
                is_private: false,
                is_tweak_mod: false,
                published_at: None,
                cover_thumb_url: None,
                metadata: serde_json::json!({"engine": "not exposed"}),
            }],
        )
        .await?;
        let mut unavailable: crate::models::vehicle::ActiveModel =
            Entity::find_by_id("XFG".parse::<crate::vehicle_id::VehicleId>()?)
                .one(&database)
                .await?
                .unwrap()
                .into();
        unavailable.available = Set(false);
        unavailable.update(&database).await?;
        let state = ApiState {
            database,
            oauth: None,
            object_store: Arc::new(object_store::memory::InMemory::new()),
            hotlaps: Default::default(),
        };

        let response = request(state.clone(), "?per_page=3&page=2").await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let page = body(response).await;
        assert_eq!(page["items"].as_array().unwrap().len(), 3);
        assert_eq!(page["items"][0]["code"], "RB4");
        assert_eq!(
            page["pagination"],
            serde_json::json!({
                "page": 2, "per_page": 3, "total_items": 20, "total_pages": 7
            })
        );

        let found = body(request(state.clone(), "?q=%20abcdef%20").await).await;
        assert_eq!(found["items"][0]["code"], "ABCDEF");
        assert_eq!(found["pagination"]["total_items"], 1);
        assert_eq!(
            found["items"][0],
            serde_json::json!({
                "code": "ABCDEF", "name": "Test vehicle", "license": "s3", "image": null
            })
        );
        let found = body(request(state.clone(), "?q=TEST").await).await;
        assert_eq!(found["pagination"]["total_items"], 1);
        for query in ["?q=XFG", "?q=%25", "?page=100"] {
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
