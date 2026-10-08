//! Reading one identified hotlap.

use super::response::Hotlap;

use crate::{
    api::{ApiError, ApiState, ErrorResponse, extractors::AuthenticatedPlayer},
    models::{
        Era, Hotlap as HotlapModel,
        era::Entity as EraEntity,
        hotlap::{self, HotlapFilter},
        player,
    },
};
use axum::{
    Json,
    extract::{Path, State},
};
use sea_orm::EntityTrait;

#[utoipa::path(
    get,
    path = "/api/v1/hotlaps/{hotlap}",
    operation_id = "get_hotlap",
    tag = "hotlaps",
    security(
        (),
        ("cookie_session" = []),
        ("personal_access_token" = [])
    ),
    params(("hotlap" = i64, Path, description = "Hotlap identity")),
    responses(
        (status = 200, description = "One hotlap, with processing fields when the caller owns it", body = Hotlap),
        (status = 401, description = "Supplied credentials are invalid", body = ErrorResponse),
        (status = 404, description = "Hotlap not found", body = ErrorResponse)
    )
)]
pub(crate) async fn detail(
    Path(hotlap_id): Path<i64>,
    State(state): State<ApiState>,
    viewer: Option<AuthenticatedPlayer>,
) -> Result<Json<Hotlap>, ApiError> {
    let viewer = viewer.map(|AuthenticatedPlayer(viewer)| viewer);
    let viewer_id = viewer.map(|viewer| viewer.id);

    // The owner lookup runs first because it is the wider one: it matches the
    // caller's own hotlaps in any state, where the public lookup only ever
    // matches validated ones.
    // The lap and its owner arrive together: the hotlap entity declares the
    // relation, so this is one join rather than a second round trip.
    let viewers_hotlap = match viewer_id {
        Some(player_id) => hotlap::Entity::find_by_id(hotlap_id)
            .uploads()
            .owned_by(player_id)
            .find_also_related(player::Entity)
            .find_also_related(EraEntity)
            .one(&state.database)
            .await
            .map_err(ApiError::database)?,
        None => None,
    };
    let owner = viewers_hotlap.is_some();
    let (hotlap, player, era) = match viewers_hotlap {
        Some(found) => found,
        None => hotlap::Entity::find_by_id(hotlap_id)
            .valid()
            .find_also_related(player::Entity)
            .find_also_related(EraEntity)
            .one(&state.database)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(hotlap_not_found)?,
    };
    // `Restrict` on the relation means a lap cannot outlive its player, so an
    // absent owner here is a broken row rather than an ordinary 404.
    let player = player.ok_or_else(|| {
        ApiError::database(sea_orm::DbErr::Type(format!(
            "hotlap {hotlap_id} has no player"
        )))
    })?;

    let era = era.ok_or_else(|| {
        ApiError::database(sea_orm::DbErr::Type(format!(
            "hotlap {hotlap_id} has no era"
        )))
    })?;

    Ok(Json(response(hotlap, player, &era, owner)))
}

fn hotlap_not_found() -> ApiError {
    ApiError::not_found("hotlap_not_found", "Hotlap")
}

fn response(hotlap: HotlapModel, player: crate::models::Player, era: &Era, owner: bool) -> Hotlap {
    let response = Hotlap::new(&hotlap, era, player.into());
    if owner {
        response.with_submission(&hotlap)
    } else {
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        milliseconds::Milliseconds,
        models::hotlap::{HotlapState, SteeringInput},
    };

    #[test]
    fn private_submission_metadata_is_only_serialized_for_the_owner() {
        let zero = Milliseconds::from_millis(0);
        let hotlap = HotlapModel {
            id: 42,
            player_id: 7,
            era_id: 1,
            chart_id: 1,
            track: crate::track_id::TrackId("BL1".parse().unwrap()),
            vehicle: "XFG".parse().unwrap(),
            raw_vehicle_name: "Private replay name".to_owned(),
            mod_version: None,
            lap_time_ms: Milliseconds::from_millis(90_000),
            split_1_ms: zero,
            split_2_ms: zero,
            split_3_ms: zero,
            split_4_ms: zero,
            original_filename: Some("private.spr".to_owned()),
            spr_object_key: Some("private/object/key".to_owned()),
            source: "upload".to_owned(),
            fingerprint: "private-fingerprint".to_owned(),
            steering: SteeringInput::Wheel,
            abs_enabled: None,
            player_flags: lfsplanet_spr::PlayerFlags::empty().into(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            game_version: crate::game_version::GameVersionCode("0.5Y".parse().unwrap()),
            state: HotlapState::Valid,
            hlvc_result_code: Some(0),
            error_detail: None,
            attempt_count: 1,
            next_attempt_at: None,
            started_at: None,
            finished_at: None,
        };
        let player = crate::models::Player {
            id: 7,
            lfs_username: "driver".to_owned(),
            display_name: "Driver".to_owned(),
            deny_auth: false,
            deny_uploads: false,
            country_code: None,
            flag_code: None,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            last_authenticated_at: None,
            lfsworld_id: None,
        };
        let era = Era {
            id: 1,
            slug: "2007-12-21".parse().unwrap(),
            title: "Historical".to_owned(),
            installation_id: "0.7".parse().unwrap(),
            version_requirement: ">=0.5Y,<0.8".parse().unwrap(),
            open: false,
        };
        let public =
            serde_json::to_value(response(hotlap.clone(), player.clone(), &era, false)).unwrap();
        let owner =
            serde_json::to_value(response(hotlap.clone(), player.clone(), &era, true)).unwrap();
        assert!(public.get("submission").is_none());
        for private in [
            "raw_vehicle_name",
            "original_filename",
            "hlvc_result_code",
            "error_detail",
            "spr_object_key",
            "fingerprint",
        ] {
            assert!(public.get(private).is_none());
            assert!(owner.get(private).is_none());
        }
        assert_eq!(owner["submission"]["original_filename"], "private.spr");
        assert_eq!(
            owner["submission"]["raw_vehicle_name"],
            "Private replay name"
        );
        assert_eq!(public["replay_url"], "/api/v1/hotlaps/42/replay");
        assert_eq!(public["player"]["id"], 7);
        assert_eq!(public["state"], "valid");
        assert_eq!(public["position"], serde_json::Value::Null);
        let make_page = || crate::models::hotlap::HotlapPage {
            entries: vec![crate::models::hotlap::HotlapListEntry {
                hotlap: hotlap.clone(),
                player: player.clone(),
                era: era.clone(),
                position: None,
                distance_to_world_record_ms: None,
                distance_to_benchmark_ms: None,
                contributes_to: vec![],
                badges: None,
            }],
            total: 1,
            chart_contributions: None,
        };
        let pagination = crate::api::PaginationQuery::default();
        for viewer in [None, Some(999)] {
            let page = serde_json::to_value(crate::api::v1::hotlaps::response::list_response(
                make_page(),
                &pagination,
                viewer,
            ))
            .unwrap();
            assert!(page["items"][0].get("submission").is_none());
        }
        let page = serde_json::to_value(crate::api::v1::hotlaps::response::list_response(
            make_page(),
            &pagination,
            Some(player.id),
        ))
        .unwrap();
        assert_eq!(
            page["items"][0]["submission"]["original_filename"],
            "private.spr"
        );
        let mut owner_public_fields = owner;
        owner_public_fields
            .as_object_mut()
            .unwrap()
            .remove("submission");
        assert_eq!(owner_public_fields, public);
    }
}
