-- Zero is only a transaction-local placeholder while PB maintenance reranks.
ALTER TABLE hotlap_personal_best ADD COLUMN position BIGINT NOT NULL DEFAULT 0;

WITH positions AS (
    SELECT pb.hotlap_id,
        ROW_NUMBER() OVER (
            PARTITION BY pb.era_id, pb.track, pb.vehicle
            ORDER BY hotlap.lap_time_ms, pb.player_id
        ) AS position
    FROM hotlap_personal_best pb
    JOIN hotlap ON hotlap.id = pb.hotlap_id
)
UPDATE hotlap_personal_best pb
SET position = positions.position
FROM positions
WHERE pb.hotlap_id = positions.hotlap_id;

CREATE INDEX hotlap_personal_best_position_idx
    ON hotlap_personal_best (era_id, track, vehicle, position);
