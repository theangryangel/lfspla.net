//! Filtered, ordered hotlap activity and the caller's submissions.

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::{
    AccessMode, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, IsolationLevel,
    PaginatorTrait, QueryFilter, QuerySelect, QueryTrait, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, Ordering, PaginatedResponse, PaginationQuery,
        extractors::AuthenticatedPlayer, v1::PlayerSummary,
    },
    models::{
        eras::EraEntity,
        hotlaps::{
            HotlapColumn, HotlapEntity, HotlapFilter, HotlapListColumn, HotlapOrder,
            HotlapRankable, HotlapState,
        },
        players::{PlayerColumn, PlayerEntity, PlayerFilter},
    },
};

use super::response::{ManagedHotlapResponse, RankingContribution};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HotlapListState {
    All,
    Unpublished,
    Pending,
    Valid,
    Invalid,
}

impl HotlapListState {
    fn state(self) -> Option<HotlapState> {
        match self {
            Self::All | Self::Unpublished => None,
            Self::Pending => Some(HotlapState::Pending),
            Self::Valid => Some(HotlapState::Valid),
            Self::Invalid => Some(HotlapState::Invalid),
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct HotlapQuery {
    /// Defaults to valid publicly, all with mine=true. Non-valid selections are caller-only.
    #[param(inline)]
    state: Option<HotlapListState>,
    /// Public era slug.
    era_id: Option<String>,
    /// Stable LFS account name (case-insensitive exact match).
    lfs_username: Option<String>,
    /// LFS track configuration code, for example BL1 or SO4R.
    #[param(value_type = Option<String>)]
    track: Option<Track>,
    /// Standard vehicle code or six-digit hexadecimal mod identifier.
    #[serde(default, deserialize_with = "deserialize_vehicle")]
    #[param(value_type = Option<String>)]
    vehicle: Option<Vehicle>,
    /// Only current ranked chart personal bests; defaults to false.
    #[serde(default)]
    ranked_only: bool,
    /// Only the caller's uploaded replays (requires authentication), including management details.
    #[serde(default)]
    mine: bool,
    /// Defaults to submitted. Unranked laps sort last in both directions.
    #[serde(default)]
    #[param(inline)]
    column: HotlapListColumn,
    /// Defaults to desc.
    #[serde(default = "descending")]
    #[param(inline)]
    order: Ordering,
}

fn deserialize_vehicle<'de, D>(deserializer: D) -> Result<Option<Vehicle>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Vehicle>::deserialize(deserializer)?
        .map(|vehicle| {
            vehicle
                .ensure_hotlap_rankable()
                .map_err(serde::de::Error::custom)
        })
        .transpose()
}

fn descending() -> Ordering {
    Ordering::Desc
}

impl HotlapQuery {
    fn filtered(&self, viewer_id: Option<i64>) -> Result<sea_orm::Select<HotlapEntity>, ApiError> {
        let (selected_state, owner) = self.scope(viewer_id)?;
        let mut hotlaps = HotlapEntity::find();
        if let Some(hotlap_state) = selected_state.state() {
            hotlaps = hotlaps.in_state(hotlap_state);
        } else if selected_state == HotlapListState::Unpublished {
            hotlaps =
                hotlaps.filter(crate::models::hotlaps::HotlapColumn::State.ne(HotlapState::Valid));
        }
        if let Some(owner) = owner {
            hotlaps = hotlaps.uploads().owned_by(owner);
        }
        if let Some(username) = &self.lfs_username {
            hotlaps = hotlaps.filter(
                HotlapColumn::PlayerId.in_subquery(
                    PlayerEntity::find()
                        .with_username(username)
                        .select_only()
                        .column(PlayerColumn::Id)
                        .into_query(),
                ),
            );
        }
        if let Some(track) = self.track {
            hotlaps = hotlaps.filter(HotlapColumn::Track.eq(track.to_string()));
        }
        if let Some(vehicle) = self.vehicle {
            hotlaps = hotlaps.filter(HotlapColumn::Vehicle.eq(vehicle.to_string()));
        }
        if self.ranked_only {
            hotlaps = hotlaps.filter(sea_orm::sea_query::Expr::cust(
                "EXISTS (SELECT 1 FROM hotlap_personal_best WHERE hotlap_id = hotlap.id)",
            ));
        }
        Ok(hotlaps)
    }

    fn scope(&self, viewer_id: Option<i64>) -> Result<(HotlapListState, Option<i64>), ApiError> {
        let state = self.state.unwrap_or(if self.mine {
            HotlapListState::All
        } else {
            HotlapListState::Valid
        });
        let owner = if self.mine || state != HotlapListState::Valid {
            Some(viewer_id.ok_or_else(|| {
                ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "authentication_required",
                    "Sign in to view your uploads.",
                )
            })?)
        } else {
            None
        };
        Ok((state, owner))
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct HotlapActivityResponse {
    /// Management details included only for mine=true; never exposed in the public feed.
    #[serde(skip_serializing_if = "Option::is_none")]
    submission: Option<ManagedHotlapResponse>,
    id: i64,
    player: PlayerSummary,
    era_id: String,
    track: String,
    #[schema(required)]
    vehicle: Option<String>,
    lap_time_ms: i64,
    /// Current chart position; null when this upload is not a ranked personal best.
    #[schema(required)]
    position: Option<i64>,
    /// Gap to the current chart world record in milliseconds; null when unranked.
    #[schema(required)]
    distance_to_world_record_ms: Option<i64>,
    contributes_to: Vec<RankingContribution>,
    state: HotlapState,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    created_at: time::OffsetDateTime,
}

#[allow(clippy::too_many_lines)]
#[utoipa::path(
    get,
    path = "/api/v1/hotlaps",
    operation_id = "list_hotlaps",
    tag = "hotlaps",
    security((), ("cookie_session" = []), ("personal_access_token" = [])),
    params(
        HotlapQuery,
        PaginationQuery
    ),
    responses(
        (status = 200, description = "Filtered hotlaps in requested order; defaults to newest first", body = PaginatedResponse<HotlapActivityResponse>),
        (status = 400, description = "Invalid filter or pagination", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid credentials", body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    Query(query): Query<HotlapQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
    viewer: Option<AuthenticatedPlayer>,
) -> Result<Json<PaginatedResponse<HotlapActivityResponse>>, ApiError> {
    let viewer = viewer.map(|AuthenticatedPlayer(viewer)| viewer);
    let offset = pagination.offset()?;
    let mut hotlaps = query.filtered(viewer.as_ref().map(|player| player.id))?;
    if let Some(era_slug) = &query.era_id {
        hotlaps = hotlaps.in_era(
            crate::api::extractors::resolve_era(&state.database, era_slug)
                .await?
                .id,
        );
    }
    let transaction = state
        .database
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(ApiError::database)?;
    let total = hotlaps
        .clone()
        .count(&transaction)
        .await
        .map_err(ApiError::database)?;
    // Keep the sort narrow: full replay/profile fields are loaded only for the page.
    let ids = hotlaps
        .ordered(query.column, query.order)
        .select_only()
        .column(HotlapColumn::Id)
        .offset(offset)
        .limit(pagination.per_page)
        .into_tuple::<i64>()
        .all(&transaction)
        .await
        .map_err(ApiError::database)?;
    let rows = HotlapEntity::find()
        .filter(HotlapColumn::Id.is_in(ids))
        .ordered(query.column, query.order)
        .find_also_related(PlayerEntity)
        // `HotlapEntity` declares `era_id` as an Era relation, so load the
        // public slug with the page rather than leaking the internal key.
        .find_also_related(EraEntity)
        .all(&transaction)
        .await
        .map_err(ApiError::database)?;
    let mut chart_data = HashMap::<i64, (i64, i64)>::new();
    let mut ranking_contributions = HashMap::<i64, Vec<RankingContribution>>::new();
    if !rows.is_empty() {
        let placeholders = (1..=rows.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let ranked = transaction
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!(
                    r"SELECT current.hotlap_id,
                             current.position,
                             current_lap.lap_time_ms - record_lap.lap_time_ms AS distance_to_world_record_ms
                      FROM hotlap_personal_best current
                      JOIN hotlap current_lap ON current_lap.id = current.hotlap_id
                      JOIN hotlap_personal_best record
                        ON record.era_id = current.era_id
                       AND record.track = current.track
                       AND record.vehicle = current.vehicle
                       AND record.position = 1
                      JOIN hotlap record_lap ON record_lap.id = record.hotlap_id
                      WHERE current.hotlap_id IN ({placeholders})"
                ),
                rows.iter().map(|(hotlap, _, _)| hotlap.id.into()),
            ))
            .await
            .map_err(ApiError::database)?;
        for row in ranked {
            chart_data.insert(
                row.try_get("", "hotlap_id").map_err(ApiError::database)?,
                (
                    row.try_get("", "position").map_err(ApiError::database)?,
                    row.try_get("", "distance_to_world_record_ms")
                        .map_err(ApiError::database)?,
                ),
            );
        }
        let ranking_rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!(
                    r"SELECT current.hotlap_id, ranking.slug, ranking.title
                       FROM hotlap_personal_best current
                       JOIN ranking_chart
                         ON ranking_chart.era_id = current.era_id
                        AND ranking_chart.track_id = current.track
                        AND ranking_chart.vehicle_id = current.vehicle
                       JOIN ranking
                         ON ranking.id = ranking_chart.ranking_id
                        AND ranking.era_id = ranking_chart.era_id
                       WHERE current.hotlap_id IN ({placeholders})
                       ORDER BY ranking.position, ranking.id"
                ),
                rows.iter().map(|(hotlap, _, _)| hotlap.id.into()),
            ))
            .await
            .map_err(ApiError::database)?;
        for row in ranking_rows {
            ranking_contributions
                .entry(row.try_get("", "hotlap_id").map_err(ApiError::database)?)
                .or_default()
                .push(RankingContribution {
                    id: row.try_get("", "slug").map_err(ApiError::database)?,
                    title: row.try_get("", "title").map_err(ApiError::database)?,
                });
        }
    }
    transaction.commit().await.map_err(ApiError::database)?;
    let activity = rows
        .into_iter()
        .map(|(hotlap, player, era)| {
            let player = player.ok_or_else(|| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "missing_player",
                    "Hotlap owner could not be loaded.",
                )
            })?;
            // The database foreign key prevents this in normal operation; a
            // missing era means the page could not be represented correctly.
            let era = era.ok_or_else(|| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "missing_era",
                    "Hotlap era could not be loaded.",
                )
            })?;
            let chart_data_for_hotlap = chart_data.get(&hotlap.id).copied();
            let contributions_for_hotlap = ranking_contributions
                .get(&hotlap.id)
                .cloned()
                .unwrap_or_default();
            let submission = query.mine.then(|| {
                let submission = ManagedHotlapResponse::new(hotlap.clone(), &era);
                let submission = match chart_data_for_hotlap {
                    Some((position, distance)) => submission.with_chart_data(position, distance),
                    None => submission,
                };
                submission.with_ranking_contributions(contributions_for_hotlap.clone())
            });
            Ok(HotlapActivityResponse {
                submission,
                id: hotlap.id,
                player: player.into(),
                era_id: era.slug,
                track: hotlap.track.to_string(),
                vehicle: hotlap.vehicle.map(|vehicle| vehicle.to_string()),
                lap_time_ms: hotlap.lap_time_ms,
                position: chart_data_for_hotlap.map(|(position, _)| position),
                distance_to_world_record_ms: chart_data_for_hotlap.map(|(_, distance)| distance),
                contributes_to: contributions_for_hotlap,
                state: hotlap.state,
                created_at: hotlap.created_at,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(PaginatedResponse {
        items: activity,
        pagination: pagination.metadata(total),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(search: &str) -> HotlapQuery {
        serde_urlencoded::from_str(search).unwrap()
    }

    #[test]
    fn typed_filters_accept_codes_and_reject_invalid_values() {
        let defaults = query("");
        assert!(defaults.track.is_none());
        assert!(defaults.vehicle.is_none());
        assert!(!defaults.ranked_only);
        let filters = query("track=SO4R&vehicle=RB4&ranked_only=true");
        assert_eq!(filters.track, Some(Track::So4r));
        assert_eq!(filters.vehicle, Some(Vehicle::Rb4));
        assert!(filters.ranked_only);
        assert_eq!(
            query("vehicle=728419").vehicle.unwrap().to_string(),
            "728419"
        );
        for search in [
            "track=ZZ9Z",
            "track=",
            "vehicle=unknown",
            "vehicle=",
            "ranked_only=yes",
        ] {
            assert!(
                serde_urlencoded::from_str::<HotlapQuery>(search).is_err(),
                "{search}"
            );
        }
    }

    #[test]
    fn filters_combine_without_changing_private_scope() {
        let filters =
            query("mine=true&lfs_username=ExAmPlE&track=SO4R&vehicle=RB4&ranked_only=true");
        assert!(filters.filtered(None).is_err());
        let sql = filters
            .filtered(Some(7))
            .unwrap()
            .build(DbBackend::Postgres)
            .to_string();
        for predicate in [
            "\"hotlap\".\"player_id\" = 7",
            "\"hotlap\".\"source\" = 'upload'",
            "LOWER(\"lfs_username\") = 'example'",
            "\"hotlap\".\"track\" = 'SO4R'",
            "\"hotlap\".\"vehicle\" = 'RB4'",
            "EXISTS (SELECT 1 FROM hotlap_personal_best WHERE hotlap_id = hotlap.id)",
        ] {
            assert!(sql.contains(predicate), "{sql}");
        }
        let sql = query("ranked_only=false")
            .filtered(None)
            .unwrap()
            .build(DbBackend::Postgres)
            .to_string();
        assert!(!sql.contains("hotlap_personal_best"));
    }

    #[test]
    fn public_defaults_and_private_scopes() {
        let public = query("");
        assert_eq!(public.column, HotlapListColumn::Submitted);
        assert_eq!(public.order, Ordering::Desc);
        assert_eq!(public.scope(None).unwrap(), (HotlapListState::Valid, None));
        assert_eq!(
            public.scope(Some(7)).unwrap(),
            (HotlapListState::Valid, None)
        );
        assert_eq!(
            query("mine=true").scope(Some(7)).unwrap(),
            (HotlapListState::All, Some(7))
        );
        assert_eq!(
            query("mine=true&state=valid").scope(Some(7)).unwrap(),
            (HotlapListState::Valid, Some(7))
        );
        for search in [
            "mine=true",
            "mine=true&state=valid",
            "state=all",
            "state=unpublished",
            "state=pending",
            "state=invalid",
        ] {
            assert!(query(search).scope(None).is_err(), "{search}");
            assert_eq!(query(search).scope(Some(7)).unwrap().1, Some(7));
        }
        for search in [
            "mine=yes",
            "column=to_wr",
            "order=invalid",
            "state=unknown",
            "state=validating",
            "state=awaiting_vehicle",
            "state=error",
        ] {
            assert!(serde_urlencoded::from_str::<HotlapQuery>(search).is_err());
        }
    }
}
