-- The upload/import row remains the complete replay history.  This table is
-- the one current, eligible personal best for each chart and player.
CREATE TABLE hotlap_personal_best (
    era_id BIGINT NOT NULL REFERENCES era (id) ON DELETE CASCADE,
    player_id BIGINT NOT NULL REFERENCES player (id) ON DELETE CASCADE,
    track TEXT NOT NULL REFERENCES track (id) ON DELETE RESTRICT,
    vehicle TEXT NOT NULL REFERENCES vehicle (id) ON DELETE RESTRICT,
    hotlap_id BIGINT NOT NULL UNIQUE REFERENCES hotlap (id) ON DELETE CASCADE,
    PRIMARY KEY (era_id, player_id, track, vehicle)
);

-- Rankings and chart leaderboards start with an era/chart lookup.
CREATE INDEX hotlap_personal_best_chart_idx
    ON hotlap_personal_best (era_id, track, vehicle, player_id);

-- Backfill with the same complete ordering used by the former DISTINCT ON
-- queries.  `id` is the final deterministic tie break.
INSERT INTO hotlap_personal_best (era_id, player_id, track, vehicle, hotlap_id)
SELECT DISTINCT ON (hotlap.era_id, hotlap.player_id, hotlap.track, hotlap.vehicle)
    hotlap.era_id,
    hotlap.player_id,
    hotlap.track,
    hotlap.vehicle,
    hotlap.id
FROM hotlap
WHERE hotlap.state = 'valid'
  AND EXISTS (
      SELECT 1
      FROM ranking_chart
      WHERE ranking_chart.era_id = hotlap.era_id
        AND ranking_chart.track_id = hotlap.track
        AND ranking_chart.vehicle_id = hotlap.vehicle
  )
ORDER BY
    hotlap.era_id,
    hotlap.player_id,
    hotlap.track,
    hotlap.vehicle,
    hotlap.lap_time_ms,
    hotlap.split_1_ms,
    hotlap.split_2_ms,
    hotlap.split_3_ms,
    hotlap.split_4_ms,
    hotlap.id;
