DROP INDEX hotlap_chart_idx;

ALTER TABLE hotlap
    DROP COLUMN era_id;

CREATE INDEX hotlap_chart_idx
    ON hotlap (track, vehicle, hlvc_status, game_version);
