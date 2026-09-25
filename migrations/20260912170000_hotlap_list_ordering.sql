-- Era upload-log pages and counts can read IDs/timestamps without loading replays.
CREATE INDEX hotlap_valid_era_submitted_idx
    ON hotlap (era_id, created_at DESC, id DESC)
    WHERE state = 'valid';

-- Lap-time ordering starts at the requested era rather than scanning every chart.
CREATE INDEX hotlap_valid_era_lap_time_idx
    ON hotlap (era_id, lap_time_ms, created_at DESC, id DESC)
    WHERE state = 'valid';

-- Rank ordering joins the era's PBs to candidate uploads. Cover the position
-- lookup so this join does not fetch every PB row from the heap.
CREATE INDEX hotlap_personal_best_era_hotlap_position_idx
    ON hotlap_personal_best (era_id, hotlap_id) INCLUDE (position);
