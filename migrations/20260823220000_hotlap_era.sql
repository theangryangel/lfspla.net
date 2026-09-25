ALTER TABLE hotlap
    ADD COLUMN era_id TEXT;

ALTER TABLE hotlap
    ALTER COLUMN era_id SET NOT NULL,
    ADD CONSTRAINT hotlap_era_id_nonempty CHECK (era_id <> '');

-- Supports fastest-valid-lap-per-player selection within an era and chart.
CREATE INDEX hotlap_valid_era_chart_best_idx
    ON hotlap (era_id, track, vehicle, player_id, lap_time_ms, id)
    WHERE hlvc_status = 'valid';

-- Era-scoped ranking queries no longer discover and filter concrete replay
-- versions, so the previous version-oriented indexes are redundant.
DROP INDEX hotlap_valid_chart_version_idx;
DROP INDEX hotlap_valid_chart_best_idx;
