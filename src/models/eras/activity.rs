//! Current chart achievements derived for one ranking era.

use sea_orm::{ConnectionTrait, DbBackend, FromQueryResult, Statement};

use super::EraModel;

/// Counts each player's current first, second, and third places across all
/// individual charts in an era.
#[derive(Debug, Clone, PartialEq, Eq, FromQueryResult)]
pub(crate) struct PodiumCounts {
    pub player_id: i64,
    /// The imported source order used to resolve equal aggregate counts.
    pub first_hotlap_id: i64,
    pub firsts: i64,
    pub seconds: i64,
    pub thirds: i64,
}

/// One of the era's leading holders of current individual-chart world records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorldRecordHolder {
    /// Competition rank by record count; equal counts share a position.
    pub position: u64,
    pub player_id: i64,
    /// Number of eligible charts on which this driver currently ranks first.
    pub world_records: i64,
}

/// Ranks an era's world record holders by record count, most records first.
///
/// Shared by the world record table and the world record leader badge so the
/// two cannot disagree about who leads. Drivers holding no record are omitted.
pub(crate) fn rank_world_record_holders(counts: &[PodiumCounts]) -> Vec<WorldRecordHolder> {
    let mut holders = counts
        .iter()
        .filter(|row| row.firsts > 0)
        .map(|row| WorldRecordHolder {
            position: 0,
            player_id: row.player_id,
            world_records: row.firsts,
        })
        .collect::<Vec<_>>();
    // `sort_by_key` is stable, so equal counts retain the source hotlap order
    // supplied by `list_podium_counts`, as PHP's `asort()` did.
    holders.sort_by_key(|holder| std::cmp::Reverse(holder.world_records));
    let mut position = 0;
    let mut previous = None;
    for (index, holder) in holders.iter_mut().enumerate() {
        if previous != Some(holder.world_records) {
            position = index as u64 + 1;
            previous = Some(holder.world_records);
        }
        holder.position = position;
    }
    holders
}

/// A player with exactly one published hotlap in an era.
#[derive(Debug, Clone, PartialEq, Eq, FromQueryResult)]
struct SingleHotlapPlayer {
    player_id: i64,
}

impl EraModel {
    /// Counts each player's current individual-chart podium positions.
    pub(crate) async fn list_podium_counts<C: ConnectionTrait>(
        &self,
        database: &C,
    ) -> Result<Vec<PodiumCounts>, sea_orm::DbErr> {
        let sql = r"
WITH player_order AS (
    -- Preserve the order in which players first appeared in the source data,
    -- matching PHP's stable asort() tie behaviour.
    SELECT hotlap.player_id, MIN(hotlap.id) AS first_hotlap_id
    FROM hotlap
    WHERE hotlap.era_id = $1
      AND hotlap.state = 'valid' AND EXISTS (SELECT 1 FROM ranking_chart WHERE era_id = hotlap.era_id AND track_id = hotlap.track AND vehicle_id = hotlap.vehicle)
    GROUP BY hotlap.player_id
)
SELECT
    podiums.player_id,
    player_order.first_hotlap_id,
    COUNT(*) FILTER (WHERE position = 1)::BIGINT AS firsts,
    COUNT(*) FILTER (WHERE position = 2)::BIGINT AS seconds,
    COUNT(*) FILTER (WHERE position = 3)::BIGINT AS thirds
FROM hotlap_personal_best podiums
JOIN player_order USING (player_id)
WHERE podiums.era_id = $1 AND position BETWEEN 1 AND 3
GROUP BY podiums.player_id, player_order.first_hotlap_id
ORDER BY player_order.first_hotlap_id, podiums.player_id
";

        PodiumCounts::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [self.id.into()],
        ))
        .all(database)
        .await
    }

    /// Lists players with exactly one published hotlap in this era.
    pub(crate) async fn list_single_hotlap_player_ids<C: ConnectionTrait>(
        &self,
        database: &C,
    ) -> Result<Vec<i64>, sea_orm::DbErr> {
        let sql = r"
SELECT player_id
FROM hotlap
WHERE era_id = $1
  AND state = 'valid' AND EXISTS (SELECT 1 FROM ranking_chart WHERE era_id = hotlap.era_id AND track_id = hotlap.track AND vehicle_id = hotlap.vehicle)
GROUP BY player_id
HAVING COUNT(*) = 1
ORDER BY player_id
";

        SingleHotlapPlayer::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [self.id.into()],
        ))
        .all(database)
        .await
        .map(|players| players.into_iter().map(|player| player.player_id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(player_id: i64, first_hotlap_id: i64, firsts: i64) -> PodiumCounts {
        PodiumCounts {
            player_id,
            first_hotlap_id,
            firsts,
            seconds: 0,
            thirds: 0,
        }
    }

    #[test]
    fn holders_rank_by_record_count() {
        let ranked =
            rank_world_record_holders(&[counts(1, 10, 2), counts(2, 20, 9), counts(3, 30, 5)]);
        assert_eq!(
            ranked
                .iter()
                .map(|holder| (holder.position, holder.player_id))
                .collect::<Vec<_>>(),
            [(1, 2), (2, 3), (3, 1)]
        );
    }

    #[test]
    fn drivers_without_a_record_are_omitted() {
        let ranked = rank_world_record_holders(&[counts(1, 10, 0), counts(2, 20, 3)]);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].player_id, 2);
        assert_eq!(ranked[0].world_records, 3);
    }

    #[test]
    fn equal_counts_share_a_position_and_skip_the_next() {
        let ranked = rank_world_record_holders(&[
            counts(1, 10, 4),
            counts(2, 20, 4),
            counts(3, 30, 4),
            counts(4, 40, 1),
        ]);
        assert_eq!(
            ranked
                .iter()
                .map(|holder| (holder.position, holder.player_id))
                .collect::<Vec<_>>(),
            [(1, 1), (1, 2), (1, 3), (4, 4)]
        );
    }

    #[test]
    fn ties_keep_the_source_hotlap_order() {
        let ranked = rank_world_record_holders(&[counts(9, 10, 6), counts(4, 20, 6)]);
        assert_eq!(
            ranked
                .iter()
                .map(|holder| holder.player_id)
                .collect::<Vec<_>>(),
            [9, 4]
        );
    }
}
