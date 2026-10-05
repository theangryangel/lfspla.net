//! Filtered, ordered hotlap activity and the caller's submissions.

use super::response::{ManagedHotlapResponse, RankingContribution};
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, Ordering, PaginatedResponse, PaginationQuery,
        extractors::AuthenticatedPlayer, v1::PlayerSummary,
    },
    models::hotlap::{
        Entity as HotlapEntity, HotlapFilter, HotlapListColumn, HotlapRankable, HotlapState,
    },
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use insim_core::{track::Track, vehicle::Vehicle};

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
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
    /// Only ranked chart personal bests at this position or better.
    rank: Option<i64>,
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
        let (state, owner) = self.scope(viewer_id)?;
        Ok(crate::models::hotlap::HotlapActivityFilter {
            state: state.state(),
            unpublished: state == HotlapListState::Unpublished,
            owner,
            lfs_username: self.lfs_username.as_deref(),
            track: self.track,
            vehicle: self.vehicle,
            ranked_only: self.ranked_only,
            rank: self.rank,
        }
        .select())
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
    let page = crate::models::hotlap::HotlapActivity::load(
        &state.database,
        hotlaps,
        query.column,
        query.order,
        offset,
        pagination.per_page,
    )
    .await
    .map_err(|error| {
        use crate::models::hotlap::ActivityError;
        match error {
            ActivityError::Database(error) => ApiError::database(error),
            ActivityError::MissingPlayer => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "missing_player",
                "Hotlap owner could not be loaded.",
            ),
            ActivityError::MissingEra => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "missing_era",
                "Hotlap era could not be loaded.",
            ),
        }
    })?;
    let total = page.total;
    let activity = page
        .entries
        .into_iter()
        .map(|entry| {
            let crate::models::hotlap::HotlapActivityEntry {
                hotlap,
                player,
                era,
                position,
                distance_to_world_record_ms,
                contributes_to,
            } = entry;
            let chart_data_for_hotlap = position.zip(distance_to_world_record_ms);
            let contributions_for_hotlap = contributes_to
                .into_iter()
                .map(Into::into)
                .collect::<Vec<RankingContribution>>();
            let submission = query.mine.then(|| {
                let submission = ManagedHotlapResponse::new(hotlap.clone(), &era);
                let submission = match chart_data_for_hotlap {
                    Some((position, distance)) => submission.with_chart_data(position, distance),
                    None => submission,
                };
                submission.with_ranking_contributions(contributions_for_hotlap.clone())
            });
            HotlapActivityResponse {
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
            }
        })
        .collect();
    Ok(Json(PaginatedResponse {
        items: activity,
        pagination: pagination.metadata(total),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, QueryTrait};

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
