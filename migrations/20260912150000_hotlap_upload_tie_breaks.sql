-- Select the earliest upload when a player's lap and split times are equal.
WITH best AS (
    SELECT DISTINCT ON (hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle)
        hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle, hotlap.id
    FROM hotlap
    JOIN hotlap_personal_best pb
      ON pb.era_id = hotlap.era_id AND pb.player_id = hotlap.player_id
     AND pb.track = hotlap.track AND pb.vehicle = hotlap.vehicle
    WHERE hotlap.state = 'valid'
    ORDER BY hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle,
             hotlap.lap_time_ms, hotlap.split_1_ms, hotlap.split_2_ms,
             hotlap.split_3_ms, hotlap.split_4_ms, hotlap.created_at, hotlap.id
)
UPDATE hotlap_personal_best pb
SET hotlap_id = best.id
FROM best
WHERE pb.era_id = best.era_id AND pb.player_id = best.player_id
  AND pb.track = best.track AND pb.vehicle = best.vehicle
  AND pb.hotlap_id IS DISTINCT FROM best.id;

-- Publish the same order across drivers, retaining username for identical dates.
WITH positions AS (
    SELECT pb.hotlap_id,
        ROW_NUMBER() OVER (
            PARTITION BY pb.era_id, pb.track, pb.vehicle
            ORDER BY hotlap.lap_time_ms, hotlap.split_1_ms, hotlap.split_2_ms,
                     hotlap.split_3_ms, hotlap.split_4_ms, hotlap.created_at,
                     player.lfs_username COLLATE "C"
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
