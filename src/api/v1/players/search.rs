//! Public player search.

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery, v1::PlayerSummary,
    },
    models::player::{Column as PlayerColumn, Entity as PlayerEntity, PlayerFilter},
};
use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use serde::Deserialize;
use utoipa::IntoParams;

/// Search text for the public driver finder.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PlayerSearchQuery {
    /// Username or display name search text.
    q: Option<String>,
    /// ISO alpha-2 country code; matches the ranking country, not a display flag.
    #[param(value_type = Option<String>)]
    country: Option<crate::country::CountryCode>,
    /// Whether the player authors an available vehicle mod.
    vehicle_author: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/v1/players",
    tag = "players",
    params(PlayerSearchQuery, PaginationQuery),
    responses(
        (status = 200, description = "Drivers in username order, optionally filtered by name and country", body = PaginatedResponse<PlayerSummary>),
        (status = 400, description = "Invalid country or pagination", body = ErrorResponse)
    )
)]
pub(crate) async fn search(
    Query(query): Query<PlayerSearchQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<PaginatedResponse<PlayerSummary>>, ApiError> {
    let offset = pagination.offset()?;
    let mut players = PlayerEntity::find();
    if let Some(query) = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        players = players.matching(query);
    }
    if let Some(country) = query.country {
        players = players.filter(PlayerColumn::CountryCode.eq(country));
    }
    if let Some(vehicle_author) = query.vehicle_author {
        use crate::models::vehicle;
        let authors = vehicle::Entity::find()
            .select_only()
            .column(vehicle::Column::AuthorPlayerId)
            .filter(vehicle::Column::AuthorPlayerId.is_not_null())
            .filter(vehicle::Column::Available.eq(true))
            .filter(vehicle::Column::Kind.eq("mod"))
            .into_query();
        players = players.filter(if vehicle_author {
            PlayerColumn::Id.in_subquery(authors)
        } else {
            PlayerColumn::Id.not_in_subquery(authors)
        });
    }
    let total_items = players
        .clone()
        .count(&state.database)
        .await
        .map_err(ApiError::database)?;
    let matches = players
        .order_by_asc(PlayerColumn::LfsUsername)
        .order_by_asc(PlayerColumn::Id)
        .offset(offset)
        .limit(pagination.per_page)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;

    Ok(Json(PaginatedResponse {
        items: matches.into_iter().map(PlayerSummary::from).collect(),
        pagination: pagination.metadata(total_items),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn request(state: &ApiState, query: &str) -> axum::response::Response {
        crate::api::router(state.hotlaps.max_spr_upload_bytes())
            .with_state(state.clone())
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/v1/players{query}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn body(state: &ApiState, query: &str) -> serde_json::Value {
        let response = request(state, query).await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn validates_country_query_codes() {
        let query: PlayerSearchQuery = serde_urlencoded::from_str("country=gb").unwrap();
        assert_eq!(query.country.unwrap().as_str(), "GB");
        for query in ["country=XX", "country=GB-SCT", "country=United%20Kingdom"] {
            assert!(serde_urlencoded::from_str::<PlayerSearchQuery>(query).is_err());
        }
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn directory_filters_before_counting_and_pagination(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        for (username, display_name, country) in [
            ("alpha", "Fast driver", Some(celes::Country::germany())),
            ("bravo", "Fast racer", Some(celes::Country::germany())),
            ("charlie", "Fast driver", Some(celes::Country::france())),
            ("delta", "Other driver", None),
            ("echo", "100% driver", Some(celes::Country::germany())),
        ] {
            crate::models::player::ActiveModel {
                lfs_username: Set(username.to_owned()),
                display_name: Set(display_name.to_owned()),
                country_code: Set(country.map(Into::into)),
                // A matching display flag must not override the selected country.
                flag_code: Set(lfsplanet_flags::FlagCode::parse("de")),
                ..Default::default()
            }
            .insert(&database)
            .await?;
        }
        crate::models::Vehicle::refresh(
            &database,
            &[
                ("AAAA01", Some("alpha")),
                ("AAAA02", Some("charlie")),
                ("AAAA03", None),
            ]
            .into_iter()
            .map(|(id, author)| lfsplanet_lfs_api::RemoteVehicleMod {
                id: id.to_owned(),
                name: id.to_owned(),
                version: 1,
                class: None,
                author_username: author.map(str::to_owned),
                work_in_progress: false,
                is_private: false,
                is_tweak_mod: false,
                published_at: None,
                cover_thumb_url: None,
                metadata: serde_json::json!({}),
            })
            .collect::<Vec<_>>(),
        )
        .await?;
        crate::models::vehicle::Entity::update_many()
            .filter(crate::models::vehicle::Column::Id.eq("AAAA02"))
            .col_expr(
                crate::models::vehicle::Column::Available,
                sea_orm::sea_query::Expr::value(false),
            )
            .exec(&database)
            .await?;
        let state = ApiState {
            site_notice: None,
            database,
            oauth: None,
            object_store: Arc::new(object_store::memory::InMemory::new()),
            hotlaps: Default::default(),
        };
        let authors = body(&state, "?vehicle_author=true&q=fast&country=DE&per_page=1").await;
        assert_eq!(authors["pagination"]["total_items"], 1);
        assert_eq!(authors["items"][0]["lfs_username"], "alpha");
        assert_eq!(
            body(&state, "?vehicle_author=true&q=charlie").await["pagination"]["total_items"],
            0
        );
        assert_eq!(
            body(&state, "?vehicle_author=false").await["pagination"]["total_items"],
            4
        );
        let all = body(&state, "?per_page=2&page=2").await;
        assert_eq!(all["pagination"]["total_items"], 5);
        assert_eq!(all["pagination"]["total_pages"], 3);
        assert_eq!(all["items"][0]["lfs_username"], "charlie");
        assert_eq!(
            body(&state, "?q=%20%20").await["pagination"]["total_items"],
            5
        );
        assert_eq!(
            body(&state, "?country=de").await["pagination"]["total_items"],
            3
        );
        let combined = body(&state, "?country=DE&q=FAST&per_page=1&page=2").await;
        assert_eq!(combined["pagination"]["total_items"], 2);
        assert_eq!(combined["items"][0]["lfs_username"], "bravo");
        assert_eq!(
            body(&state, "?q=CHARLIE").await["items"][0]["lfs_username"],
            "charlie"
        );
        assert_eq!(body(&state, "?q=%25").await["pagination"]["total_items"], 1);
        assert_eq!(
            body(&state, "?country=GB").await["pagination"]["total_items"],
            0
        );
        let beyond = body(&state, "?page=100").await;
        assert!(beyond["items"].as_array().unwrap().is_empty());
        assert_eq!(beyond["pagination"]["total_items"], 5);
        for query in ["?country=XX", "?page=0", "?per_page=101"] {
            assert_eq!(
                request(&state, query).await.status(),
                axum::http::StatusCode::BAD_REQUEST
            );
        }
        Ok(())
    }
}
