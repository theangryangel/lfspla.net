-- Publish chart ties by split times, then LFS username, independently of player IDs.
WITH positions AS (
    SELECT pb.hotlap_id,
        ROW_NUMBER() OVER (
            PARTITION BY pb.era_id, pb.track, pb.vehicle
            ORDER BY hotlap.lap_time_ms, hotlap.split_1_ms, hotlap.split_2_ms,
                     hotlap.split_3_ms, hotlap.split_4_ms, player.lfs_username COLLATE "C"
        ) AS position
    FROM hotlap_personal_best pb
    JOIN hotlap ON hotlap.id = pb.hotlap_id
    JOIN player ON player.id = pb.player_id
)
UPDATE hotlap_personal_best pb
SET position = positions.position
FROM positions
WHERE pb.hotlap_id = positions.hotlap_id
  AND pb.position IS DISTINCT FROM positions.position;
