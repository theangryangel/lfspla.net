//! Public vehicle catalogue, independent of era chart selections.

use crate::{
    api::{ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery},
    models::vehicle::{Column, Entity, VehicleFilter},
};
use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};
use utoipa_axum::{router::OpenApiRouter, routes};

mod query;
mod response;
use query::VehicleQuery;
use response::ExplorerVehicle;

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
        (status = 200, description = "Available standard vehicles and mods with cached specifications", body = PaginatedResponse<ExplorerVehicle>),
        (status = 400, description = "Invalid filters or pagination", body = ErrorResponse)
    )
)]
pub(crate) async fn list_vehicles(
    Query(query): Query<VehicleQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<PaginatedResponse<ExplorerVehicle>>, ApiError> {
    query.validate()?;
    let offset = pagination.offset()?;
    let search = query.q.as_deref().unwrap_or("");
    let vehicles = query.filter(
        Entity::find()
            .filter(Column::Available.eq(true))
            .name_or_id_contains(search),
    );
    let total_items = vehicles
        .clone()
        .count(&state.database)
        .await
        .map_err(ApiError::database)?;
    let vehicles = query
        .order(vehicles)
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
            .map(|v| ExplorerVehicle::from_record(v, &attachments))
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
    async fn top_rated_weights_votes_before_filtering_and_pagination(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        crate::services::catalogue::fixtures::vehicles(&database).await?;
        let template = Entity::find_by_id("XFG".parse::<crate::vehicle_id::VehicleId>()?)
            .one(&database)
            .await?
            .unwrap();
        for (id, name, rating, votes, available, kind) in [
            (
                "AAAA01",
                "Candidate proven",
                serde_json::json!(4.8),
                serde_json::json!(200),
                true,
                "mod",
            ),
            (
                "AAAA02",
                "Candidate new",
                serde_json::json!(5),
                serde_json::json!(1),
                true,
                "mod",
            ),
            (
                "AAAA03",
                "Baseline",
                serde_json::json!(3),
                serde_json::json!(1000),
                true,
                "mod",
            ),
            (
                "AAAA04",
                "Unrated",
                serde_json::json!(5),
                serde_json::json!(0),
                true,
                "mod",
            ),
            (
                "AAAA05",
                "Malformed votes",
                serde_json::json!(5),
                serde_json::json!("many"),
                true,
                "mod",
            ),
            (
                "AAAA06",
                "Malformed rating",
                serde_json::json!("unknown"),
                serde_json::json!(10),
                true,
                "mod",
            ),
            (
                "AAAA07",
                "Hidden",
                serde_json::json!(5),
                serde_json::json!(1_000_000),
                false,
                "mod",
            ),
            (
                "XFG",
                "Standard",
                serde_json::json!(5),
                serde_json::json!(1_000_000),
                true,
                "standard",
            ),
        ] {
            let mut vehicle: crate::models::vehicle::ActiveModel = template.clone().into();
            vehicle.id = Set(id.parse()?);
            vehicle.name = Set(name.to_owned());
            vehicle.normalized_name = Set(name.to_lowercase());
            vehicle.kind = Set(kind.to_owned());
            vehicle.available = Set(available);
            vehicle.metadata = Set(Some(serde_json::json!({
                "id": id, "name": name, "version": 1, "wip": false,
                "private": false, "tweakMod": false, "approved": true, "ev": false,
                "rating": rating, "numRatings": votes, "numDownloads": 0,
            })));
            if kind == "standard" {
                vehicle.update(&database).await?;
            } else {
                vehicle.insert(&database).await?;
            }
        }
        let state = ApiState {
            site_notice: None,
            database,
            oauth: None,
            object_store: Arc::new(object_store::memory::InMemory::new()),
            hotlaps: Default::default(),
        };
        let ranked = body(request(state.clone(), "?kind=mod&sort=rating").await).await;
        let codes: Vec<_> = ranked["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["code"].as_str().unwrap())
            .collect();
        assert_eq!(
            codes,
            ["AAAA01", "AAAA02", "AAAA03", "AAAA04", "AAAA05", "AAAA06"]
        );
        // The response still exposes the actual average and count, not the score.
        assert_eq!(ranked["items"][0]["rating"], 4.8);
        assert_eq!(ranked["items"][0]["num_ratings"], 200);
        // Searching must not recalculate the prior from just these two results.
        let filtered = body(request(state.clone(), "?q=Candidate&sort=rating").await).await;
        assert_eq!(filtered["items"][0]["code"], "AAAA01");
        assert_eq!(filtered["pagination"]["total_items"], 2);
        let second = body(request(state, "?kind=mod&sort=rating&per_page=1&page=2").await).await;
        assert_eq!(second["items"][0]["code"], "AAAA02");
        assert_eq!(second["pagination"]["total_items"], 6);
        Ok(())
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn catalogue_search_and_pagination_without_eras(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        crate::services::catalogue::fixtures::vehicles(&database).await?;
        crate::models::Vehicle::refresh(
            &database,
            &[lfsplanet_lfs_api::RemoteVehicleMod {
                id: "ABCDEF".to_owned(),
                name: "Test vehicle".to_owned(),
                version: 1,
                class: Some(2),
                author_username: Some("Rubie".to_owned()),
                work_in_progress: false,
                is_private: false,
                is_tweak_mod: false,
                published_at: None,
                cover_thumb_url: None,
                metadata: serde_json::json!({
                    "id": "ABCDEF", "name": "Test vehicle", "wip": false,
                    "private": false, "tweakMod": false,
                    "version": 1, "approved": true, "ev": false,
                    "rating": 4.8, "numRatings": 20, "numDownloads": 100,
                    "vehicle": {"drive": 1, "shiftType": 1, "power": 160,
                        "mass": 1425, "torque": 275, "iceCc": 1998, "powerWeightRatio": 112,
                        "iceNumCylinders": 6, "iceLayout": 0, "maxPowerRpm": 6416,
                        "maxTorqueRpm": 4512, "fuelTankSize": 65}
                }),
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
            site_notice: None,
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
        assert_eq!(found["items"][0]["specifications"]["power_kw"], 160.0);
        assert_eq!(found["items"][0]["author"], "Rubie");
        assert!(found["items"][0].get("metadata").is_none());
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

        let filtered = body(request(state.clone(),
            "?class=2&author=rubie&approved=true&electric=false&drive=1&transmission=1&min_power_kw=150&max_mass_kg=1500&sort=power"
        ).await).await;
        assert_eq!(filtered["pagination"]["total_items"], 1);
        for query in [
            "?drive=2",
            "?max_power_kw=100",
            "?min_mass_kg=1500",
            "?electric=true",
            "?approved=false",
        ] {
            assert_eq!(
                body(request(state.clone(), query).await).await["pagination"]["total_items"],
                0
            );
        }
        // Unknown specs do not match ranges; malformed JSON numbers also stay unknown.
        Entity::update_many()
            .filter(Column::Id.eq("ABCDEF"))
            .col_expr(
                Column::Metadata,
                sea_orm::sea_query::Expr::value(
                    serde_json::json!({"vehicle": {"power": "unknown"}}),
                ),
            )
            .exec(&state.database)
            .await?;
        assert_eq!(
            body(request(state.clone(), "?min_power_kw=0").await).await["pagination"]["total_items"],
            0
        );
        assert_eq!(
            body(request(state.clone(), "?sort=power&per_page=100").await).await["items"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        for query in [
            "?drive=4",
            "?class=15",
            "?min_power_kw=NaN",
            "?min_mass_kg=2&max_mass_kg=1",
            "?sort=bogus",
        ] {
            assert_eq!(
                request(state.clone(), query).await.status(),
                axum::http::StatusCode::BAD_REQUEST
            );
        }
        Ok(())
    }
}
