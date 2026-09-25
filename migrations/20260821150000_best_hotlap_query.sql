ALTER TABLE hotlap
    ADD COLUMN lap_time_ms BIGINT;

-- SPR split times are cumulative and unused split columns contain zero.
UPDATE hotlap
SET lap_time_ms = GREATEST(split_1_ms, split_2_ms, split_3_ms, split_4_ms);

ALTER TABLE hotlap
    ALTER COLUMN lap_time_ms SET NOT NULL,
    ADD CONSTRAINT hotlap_lap_time_positive CHECK (lap_time_ms > 0);

DROP INDEX hotlap_chart_idx;

-- Resolves the concrete replay versions present for a requested chart. The
-- application maps those versions to its checked-in era definitions.
CREATE INDEX hotlap_valid_chart_version_idx
    ON hotlap (track, vehicle, game_version)
    WHERE hlvc_status = 'valid';

-- Supports fastest-valid-lap-per-player selection and deterministic ordering.
CREATE INDEX hotlap_valid_chart_best_idx
    ON hotlap (track, vehicle, player_id, lap_time_ms, id)
    INCLUDE (game_version)
    WHERE hlvc_status = 'valid';
