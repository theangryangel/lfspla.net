//! Filtered, ordered hotlap activity and the caller's submissions.

use super::{
    query::{controller_filter, country_filter},
    response::{Hotlap, list_response},
};
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, Ordering, PaginatedResponse, PaginationQuery,
        extractors::AuthenticatedPlayer,
    },
    models::{
        Hotlap as HotlapModel,
        hotlap::{HotlapListColumn, HotlapListFilters, HotlapListPage, HotlapState},
    },
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use insim_core::{track::Track, vehicle::Vehicle};

use serde::Deserialize;
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
    /// ISO 3166-1 alpha-2 player country.
    country: Option<String>,
    /// Controller: wheel, mouse, keyboard, or keyboard_stabilised.
    controller: Option<String>,
    /// Only the caller's uploaded replays (requires authentication), including management details.
    #[serde(default)]
    mine: bool,
    /// Defaults to submitted. Ties use chart rank ascending; unranked ties sort last.
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
            if vehicle == Vehicle::Unknown {
                Err(serde::de::Error::custom(
                    "identifier cannot participate in a ranked hotlap",
                ))
            } else {
                Ok(vehicle)
            }
        })
        .transpose()
}

fn descending() -> Ordering {
    Ordering::Desc
}

impl HotlapQuery {
    fn filters(&self, viewer_id: Option<i64>) -> Result<HotlapListFilters<'_>, ApiError> {
        let (state, owner) = self.scope(viewer_id)?;
        Ok(HotlapListFilters {
            state: state.state(),
            unpublished: state == HotlapListState::Unpublished,
            owner,
            lfs_username: self.lfs_username.as_deref(),
            track: self.track,
            vehicle: self.vehicle,
            ranked_only: self.ranked_only,
            rank: self.rank,
            country: country_filter(self.country.as_deref())?,
            controller: controller_filter(self.controller.as_deref())?,
            ..HotlapListFilters::default()
        })
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
        (status = 200, description = "Filtered hotlaps in requested order; defaults to newest first", body = PaginatedResponse<Hotlap>),
        (status = 400, description = "Invalid filter or pagination", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid credentials", body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    Query(query): Query<HotlapQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
    viewer: Option<AuthenticatedPlayer>,
) -> Result<Json<PaginatedResponse<Hotlap>>, ApiError> {
    let viewer = viewer.map(|AuthenticatedPlayer(viewer)| viewer);
    let offset = pagination.offset()?;
    let viewer_id = viewer.as_ref().map(|player| player.id);
    let mut filters = query.filters(viewer_id)?;
    if let Some(era_slug) = &query.era_id {
        filters.era_id = Some(
            crate::api::extractors::resolve_era(&state.database, era_slug)
                .await?
                .id,
        );
    }
    let page = HotlapModel::list(
        &state.database,
        filters,
        HotlapListPage {
            column: query.column,
            order: query.order,
            offset,
            limit: pagination.per_page,
        },
    )
    .await
    .map_err(ApiError::from)?;
    Ok(Json(list_response(
        page,
        &pagination,
        query.mine.then_some(viewer_id).flatten(),
    )))
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
        assert!(filters.filters(None).is_err());
        let sql = filters
            .filters(Some(7))
            .unwrap()
            .select()
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
            .filters(None)
            .unwrap()
            .select()
            .build(DbBackend::Postgres)
            .to_string();
        assert!(!sql.contains("hotlap_personal_best"));
    }

    #[test]
    fn common_country_and_controller_filters_are_validated() {
        let request = query("country=GB&controller=mouse");
        let filters = request.filters(None).unwrap();
        assert_eq!(filters.country.unwrap().alpha2, "GB");
        assert_eq!(
            filters.controller,
            Some(crate::models::hotlap::SteeringInput::Mouse)
        );
        assert!(query("country=invalid").filters(None).is_err());
        assert!(query("controller=invalid").filters(None).is_err());
        assert_eq!(query("column=set").column, HotlapListColumn::Submitted);
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
