//! Immediate hotlap validation for test environments.

use super::response::ManagedHotlapResponse;
use crate::{
    api::{ApiError, ApiState, ErrorResponse, extractors::AuthenticatedPlayer},
    models::{badge::rebuild_published_badges, hotlap::TestValidationError},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

#[utoipa::path(
    post,
    path = "/api/v1/hotlaps/{hotlap}/validate",
    tag = "hotlaps",
    security(
        ("cookie_session" = []),
        ("personal_access_token" = [])
    ),
    params(
        ("hotlap" = i64, Path, description = "Hotlap identity"),
        ("X-CSRF-Token" = String, Header, description = "Required with cookie-session authentication")
    ),
    responses(
        (status = 200, description = "Hotlap immediately marked valid", body = ManagedHotlapResponse),
        (status = 401, description = "Authentication required", body = ErrorResponse),
        (status = 403, description = "CSRF token invalid or test validation disabled", body = ErrorResponse),
        (status = 404, description = "Hotlap not found", body = ErrorResponse)
    )
)]
pub(crate) async fn validate_for_testing(
    Path(hotlap_id): Path<i64>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
) -> Result<Json<ManagedHotlapResponse>, ApiError> {
    if !state.hotlaps.allow_test_validation {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "test_validation_disabled",
            "Immediate test validation is disabled",
        ));
    }

    let (hotlap, era) =
        crate::models::Hotlap::validate_owned_for_testing(&state.database, hotlap_id, player.id)
            .await
            .map_err(|error| match error {
                TestValidationError::UnsupportedCombination => ApiError::new(
                    StatusCode::CONFLICT,
                    "combination_not_eligible",
                    "The hotlap's combination is not eligible for this era",
                ),
                TestValidationError::Database(error) => ApiError::database(error),
            })?
            .ok_or_else(|| ApiError::not_found("hotlap_not_found", "Hotlap"))?;
    // Rebuild badges after publishing, as the background validator does.
    rebuild_published_badges(&state.database, hotlap.era_id).await;

    Ok(Json(ManagedHotlapResponse::new(hotlap, &era)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::EntityTrait;

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn transactional_validation_retains_conflict_and_owner_scope(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        sqlx::raw_sql(include_str!(
            "../../../services/validate_hotlap/fixtures.sql"
        ))
        .execute(&pool)
        .await?;
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let player = crate::models::player::Entity::find()
            .one(&database)
            .await?
            .unwrap();
        let hotlap = crate::models::hotlap::Entity::find()
            .one(&database)
            .await?
            .unwrap();
        let state = ApiState {
            database,
            oauth: None,
            object_store: std::sync::Arc::new(object_store::memory::InMemory::new()),
            hotlaps: crate::settings::HotlapSettings {
                allow_test_validation: true,
                ..Default::default()
            },
        };
        sqlx::query("UPDATE vehicle SET available = false WHERE id = 'XFG'")
            .execute(&pool)
            .await?;
        let error = validate_for_testing(
            Path(hotlap.id),
            State(state.clone()),
            AuthenticatedPlayer(player.clone()),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.code, "combination_not_eligible");
        let status: String = sqlx::query_scalar("SELECT state FROM hotlap")
            .fetch_one(&pool)
            .await?;
        assert_eq!(status, "pending");
        let mut other = player.clone();
        other.id += 1;
        let error = validate_for_testing(
            Path(hotlap.id),
            State(state.clone()),
            AuthenticatedPlayer(other),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        sqlx::query("UPDATE vehicle SET available = true WHERE id = 'XFG'")
            .execute(&pool)
            .await?;
        let _ = validate_for_testing(Path(hotlap.id), State(state), AuthenticatedPlayer(player))
            .await?;
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT state FROM hotlap")
                .fetch_one(&pool)
                .await?,
            "valid"
        );
        Ok(())
    }
}
