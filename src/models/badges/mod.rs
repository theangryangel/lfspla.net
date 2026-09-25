//! Calculates, stores, and loads player badges for each era.

mod entity;
mod filter;

pub(crate) use filter::BadgeFilter;

pub(crate) use entity::*;
pub use entity::{
    ActiveModel as PlayerEraBadgesMutation, Column as PlayerEraBadgesColumn,
    Entity as PlayerEraBadgesEntity, Model as PlayerEraBadgesModel,
};

use sea_orm::{ActiveValue::Set, DatabaseTransaction, EntityTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A badge currently held by a player in one era.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum PlayerBadge {
    /// Current position in an era ranking configured to award a badge.
    Ranking {
        ranking_id: String,
        label: String,
        title: String,
        ranking_position: i64,
    },
    /// Every chart in an era ranking has a published result.
    RankingCompletion {
        ranking_id: String,
        label: String,
        title: String,
    },
    /// One or more individual-chart podium positions.
    WorldRecordPodium {
        level: PodiumLevel,
        firsts: i64,
        seconds: i64,
        thirds: i64,
    },
    /// One of the era's leading holders of current world records.
    WorldRecordLeader {
        leader_position: i64,
        world_records: i64,
    },
    /// Exactly one published hotlap in this era.
    Newbie,
}

/// The highest individual-chart podium position currently held by a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PodiumLevel {
    Gold,
    Silver,
    Bronze,
}

/// Replaces badges inside the transaction that calculated them.
pub(crate) async fn replace_era(
    transaction: &DatabaseTransaction,
    era_id: i64,
    badges: std::collections::HashMap<i64, Vec<PlayerBadge>>,
) -> Result<usize, sea_orm::DbErr> {
    PlayerEraBadgesEntity::delete_many()
        .in_era(era_id)
        .exec(transaction)
        .await?;

    let count = badges.len();
    if !badges.is_empty() {
        let models = badges
            .into_iter()
            .map(|(player_id, badges)| {
                let badges = serde_json::to_value(badges).map_err(|error| {
                    sea_orm::DbErr::Type(format!("failed to serialize player badges: {error}"))
                })?;
                Ok(PlayerEraBadgesMutation {
                    era_id: Set(era_id),
                    player_id: Set(player_id),
                    badges: Set(badges),
                })
            })
            .collect::<Result<Vec<_>, sea_orm::DbErr>>()?;
        PlayerEraBadgesEntity::insert_many(models)
            .exec(transaction)
            .await?;
    }

    Ok(count)
}

pub(crate) fn decode(row: PlayerEraBadgesModel) -> Result<Vec<PlayerBadge>, sea_orm::DbErr> {
    serde_json::from_value(row.badges).map_err(|error| {
        sea_orm::DbErr::Type(format!(
            "invalid badges stored for player {} in era {}: {error}",
            row.player_id, row.era_id
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        eras::qualifies_for_ranking_badge,
        rankings::{RankingBadgeDefinition, RankingBadgeQualification},
    };

    #[test]
    fn player_badges_round_trip_through_json() {
        let badges = vec![
            PlayerBadge::Ranking {
                ranking_id: "mhr".to_owned(),
                label: "MHR".to_owned(),
                title: "Main Hotlap Rank".to_owned(),
                ranking_position: 42,
            },
            PlayerBadge::WorldRecordPodium {
                level: PodiumLevel::Gold,
                firsts: 3,
                seconds: 7,
                thirds: 12,
            },
            PlayerBadge::WorldRecordLeader {
                leader_position: 2,
                world_records: 3,
            },
            PlayerBadge::Newbie,
        ];
        let encoded = serde_json::to_value(&badges).unwrap();
        assert_eq!(
            serde_json::from_value::<Vec<PlayerBadge>>(encoded).unwrap(),
            badges
        );
    }

    #[test]
    fn ranking_badges_keep_their_stable_wire_shape() {
        let encoded = serde_json::json!({
            "kind": "ranking",
            "ranking_id": "mhr",
            "label": "MHR",
            "title": "Main Hotlap Rank",
            "ranking_position": 42
        });
        let badge = serde_json::from_value::<PlayerBadge>(encoded).unwrap();

        assert_eq!(
            badge,
            PlayerBadge::Ranking {
                ranking_id: "mhr".to_owned(),
                label: "MHR".to_owned(),
                title: "Main Hotlap Rank".to_owned(),
                ranking_position: 42,
            }
        );
    }

    #[test]
    fn world_record_leader_badges_keep_their_stable_wire_shape() {
        assert_eq!(
            serde_json::to_value(PlayerBadge::WorldRecordLeader {
                leader_position: 1,
                world_records: 14,
            })
            .unwrap(),
            serde_json::json!({
                "kind": "world_record_leader",
                "leader_position": 1,
                "world_records": 14
            })
        );
    }

    #[test]
    fn newbie_badge_has_a_stable_wire_shape() {
        assert_eq!(
            serde_json::to_value(PlayerBadge::Newbie).unwrap(),
            serde_json::json!({ "kind": "newbie" })
        );
    }

    #[test]
    fn complete_badges_require_every_chart() {
        assert!(!qualifies_for_ranking_badge(
            RankingBadgeQualification::Complete,
            17,
            18,
            0
        ));
        assert!(qualifies_for_ranking_badge(
            RankingBadgeQualification::Complete,
            18,
            18,
            0
        ));
    }

    #[test]
    fn completion_awards_do_not_expand_the_primary_award() {
        let definition: RankingBadgeDefinition = serde_json::from_value(serde_json::json!({
            "label": "NUTTER",
            "qualification": { "type": "ranked", "limit": 3 },
            "completion": { "label": "GABOR" }
        }))
        .unwrap();
        assert_eq!(definition.result_limit(), u64::MAX);
        for (index, completed, primary, completion) in [
            (0, 18, true, true),
            (2, 17, true, false),
            (3, 18, false, true),
            (20, 17, false, false),
        ] {
            assert_eq!(
                qualifies_for_ranking_badge(definition.qualification, completed, 18, index),
                primary
            );
            assert_eq!(
                qualifies_for_ranking_badge(
                    RankingBadgeQualification::Complete,
                    completed,
                    18,
                    index
                ),
                completion
            );
        }
        let legacy: RankingBadgeDefinition = serde_json::from_value(serde_json::json!({
            "label": "NUTTER",
            "qualification": { "type": "ranked", "limit": 3 }
        }))
        .unwrap();
        assert_eq!(legacy.result_limit(), 3);
        assert!(legacy.completion.is_none());
    }

    #[test]
    fn completion_badge_has_a_stable_wire_shape() {
        let badge = PlayerBadge::RankingCompletion {
            ranking_id: "nutter".into(),
            label: "GABOR".into(),
            title: "Nutter Rank".into(),
        };
        let encoded = serde_json::json!({
            "kind": "ranking_completion",
            "ranking_id": "nutter",
            "label": "GABOR",
            "title": "Nutter Rank"
        });
        assert_eq!(serde_json::to_value(&badge).unwrap(), encoded);
        assert_eq!(
            serde_json::from_value::<PlayerBadge>(encoded).unwrap(),
            badge
        );
    }

    #[test]
    fn ranked_badges_bound_the_requested_results() {
        let limited: RankingBadgeQualification =
            serde_json::from_str(r#"{"type":"ranked","limit":10}"#).unwrap();
        let unlimited: RankingBadgeQualification =
            serde_json::from_str(r#"{"type":"ranked"}"#).unwrap();

        assert_eq!(limited.result_limit(), 10);
        assert_eq!(unlimited.result_limit(), u64::MAX);
        assert_eq!(RankingBadgeQualification::Complete.result_limit(), u64::MAX);
    }
}
