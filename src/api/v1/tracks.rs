//! Public track catalogue, independent of era chart selections.

use crate::{
    api::{ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery},
    models::track::{Entity, TrackFilter, TrackOrder},
};
use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::{eras::tracks::TrackSummary, track_locations::TrackLocationSummary};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TrackQuery {
    /// Case-insensitive track code or name search. Omit to browse all configurations.
    q: Option<String>,
    /// Restrict configurations to this location code.
    location: Option<String>,
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
    let mut tracks = Entity::find().name_or_id_contains(query.q.as_deref().unwrap_or(""));
    if let Some(location) = query.location {
        tracks = tracks.filter(
            crate::models::track::Column::Location.eq(location.trim().to_ascii_uppercase()),
        );
    }
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
    let attachments = super::attachments::load(
        &state.database,
        tracks
            .iter()
            .flat_map(|track| [track.image_attachment_id, track.outline_attachment_id])
            .flatten()
            .collect::<Vec<_>>(),
    )
    .await?;
    let locations = TrackLocationSummary::load(&state.database).await?;
    Ok(Json(PaginatedResponse {
        items: tracks
            .into_iter()
            .map(|track| TrackSummary::from_record(track, &attachments, &locations))
            .collect::<Result<Vec<_>, _>>()?,
        pagination: pagination.metadata(total_items),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn request(state: ApiState, query: &str) -> axum::response::Response {
        request_path(state, &format!("/api/v1/tracks{query}")).await
    }

    async fn request_path(state: ApiState, path: &str) -> axum::response::Response {
        crate::api::router(state.hotlaps.max_spr_upload_bytes())
            .with_state(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri(path)
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
        crate::services::catalogue::fixtures::tracks(&database).await?;
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
            serde_json::json!({"code": "BL", "name": "Blackwood", "description": null, "sequence": 0, "image": null})
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
        let filtered = body(request(state.clone(), "?location=%20bl%20&per_page=3").await).await;
        assert_eq!(filtered["items"].as_array().unwrap().len(), 3);
        assert!(
            filtered["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|track| track["location"]["code"] == "BL")
        );
        let matching = tracks
            .iter()
            .filter(|track| crate::models::track::location::canonical_code(**track) == "BL")
            .count();
        assert_eq!(filtered["pagination"]["total_items"], matching);
        let next = body(request(state.clone(), "?location=BL&per_page=3&page=2").await).await;
        assert_ne!(filtered["items"][0]["code"], next["items"][0]["code"]);
        for query in [
            "?location=BL&q=FE1",
            "?location=unknown",
            "?q=ZZ9",
            "?q=%25",
            "?q=_",
            "?page=10000",
        ] {
            let found = body(request(state.clone(), query).await).await;
            assert!(found["items"].as_array().unwrap().is_empty());
        }
        for query in ["?page=0", "?per_page=101", "?page=abc"] {
            assert_eq!(
                request(state.clone(), query).await.status(),
                axum::http::StatusCode::BAD_REQUEST
            );
        }
        // Authored location metadata and artwork flow from seed into public tracks.
        let directory = tempfile::tempdir()?;
        let yaml = directory.path().join("location.yaml");
        std::fs::write(directory.path().join("location.svg"), "<svg/>")?;
        std::fs::write(
            &yaml,
            "kind: Location\nid: BL\nname: Authored Blackwood\ndescription: Shared description\nsequence: 42\nimage: ./location.svg\ntracks:\n  - id: BL1\n",
        )?;
        let documents = crate::services::catalogue::load::files(std::slice::from_ref(&yaml))?;
        let preview =
            crate::services::catalogue::seed(&state.database, &documents, None, true, |_| Ok(()))
                .await?;
        assert_eq!(preview.locations, ["BL"]);
        let before = body(request(state.clone(), "?q=BL1").await).await;
        assert_eq!(before["items"][0]["location"]["name"], "Blackwood");
        crate::services::catalogue::seed(
            &state.database,
            &documents,
            Some(state.object_store.as_ref()),
            false,
            |_| Ok(()),
        )
        .await?;
        let authored = body(request(state.clone(), "?q=BL1").await).await;
        let location = &authored["items"][0]["location"];
        assert_eq!(location["name"], "Authored Blackwood");
        assert_eq!(location["description"], "Shared description");
        assert_eq!(location["sequence"], 42);
        // The independent location catalogue exposes the same authored metadata,
        // with stable ordering and no era requirement.
        let catalogue = body(request_path(state.clone(), "/api/v1/track-locations").await).await;
        let locations = catalogue["items"].as_array().unwrap();
        assert_eq!(locations.len(), 8);
        assert_eq!(locations[0]["code"], "SO");
        assert_eq!(locations.last().unwrap(), location);
        let detail = body(request_path(state.clone(), "/api/v1/track-locations/bl").await).await;
        assert_eq!(&detail, location);
        let missing = request_path(state.clone(), "/api/v1/track-locations/UNKNOWN").await;
        assert_eq!(missing.status(), axum::http::StatusCode::NOT_FOUND);

        assert!(
            locations
                .windows(2)
                .all(|pair| pair[0]["sequence"].as_i64() <= pair[1]["sequence"].as_i64())
        );

        assert!(
            location["image"]["url"]
                .as_str()
                .unwrap()
                .starts_with("/api/v1/attachments/")
        );
        let repeated = crate::services::catalogue::seed(
            &state.database,
            &documents,
            Some(state.object_store.as_ref()),
            false,
            |_| Ok(()),
        )
        .await?;
        assert!(repeated.locations.is_empty());
        std::fs::write(&yaml, "kind: Location\nid: BL\nname: Authored Blackwood\n")?;
        let omitted = crate::services::catalogue::load::files(&[yaml])?;
        crate::services::catalogue::seed(
            &state.database,
            &omitted,
            Some(state.object_store.as_ref()),
            false,
            |_| Ok(()),
        )
        .await?;
        let preserved = body(request(state.clone(), "?q=BL1").await).await;
        assert_eq!(preserved["items"][0]["location"], *location);
        Ok(())
    }
}
