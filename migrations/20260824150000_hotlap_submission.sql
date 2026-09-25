CREATE TABLE hotlap_submission (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    player_id BIGINT NOT NULL REFERENCES player (id) ON DELETE RESTRICT,
    era_id TEXT NOT NULL CHECK (era_id <> ''),
    track TEXT NOT NULL CHECK (btrim(track) <> ''),
    vehicle TEXT,
    raw_vehicle_name TEXT NOT NULL CHECK (btrim(raw_vehicle_name) <> ''),
    mod_version SMALLINT CHECK (mod_version BETWEEN 0 AND 999),
    lap_time_ms BIGINT NOT NULL CHECK (lap_time_ms > 0),
    split_1_ms BIGINT NOT NULL CHECK (split_1_ms >= 0),
    split_2_ms BIGINT NOT NULL CHECK (split_2_ms >= 0),
    split_3_ms BIGINT NOT NULL CHECK (split_3_ms >= 0),
    split_4_ms BIGINT NOT NULL CHECK (split_4_ms >= 0),
    original_filename TEXT NOT NULL CHECK (btrim(original_filename) <> ''),
    spr_object_key TEXT NOT NULL CHECK (btrim(spr_object_key) <> ''),
    fingerprint TEXT NOT NULL UNIQUE CHECK (fingerprint ~ '^[0-9a-f]{64}$'),
    steering TEXT NOT NULL CHECK (
        steering IN ('wheel', 'mouse', 'keyboard', 'keyboard_stabilised')
    ),
    brake_help_enabled BOOLEAN NOT NULL,
    automatic_gears BOOLEAN NOT NULL,
    manual_shifter BOOLEAN NOT NULL,
    axis_clutch BOOLEAN NOT NULL,
    automatic_clutch BOOLEAN NOT NULL,
    driver_side TEXT NOT NULL CHECK (driver_side IN ('left', 'right')),
    abs_enabled BOOLEAN NOT NULL,
    game_version TEXT NOT NULL CHECK (btrim(game_version) <> ''),
    hlvc_status TEXT NOT NULL DEFAULT 'pending' CHECK (
        hlvc_status IN ('pending', 'validating', 'valid', 'invalid', 'error')
    ),
    hlvc_result_code SMALLINT CHECK (hlvc_result_code BETWEEN 0 AND 20),
    error_detail TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX hotlap_submission_player_created_idx
    ON hotlap_submission (player_id, created_at DESC);

CREATE INDEX hotlap_submission_pending_idx
    ON hotlap_submission (next_attempt_at, created_at)
    WHERE hlvc_status IN ('pending', 'error');

-- Preserve existing uploaded rows as submissions. Previously their vehicle was
-- derived from the filename; retain that identity during this structural
-- migration.
INSERT INTO hotlap_submission (
    player_id, era_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    original_filename, spr_object_key, fingerprint, steering,
    brake_help_enabled, automatic_gears, manual_shifter, axis_clutch,
    automatic_clutch, driver_side, abs_enabled, game_version, hlvc_status,
    created_at
)
SELECT
    player_id, era_id, track, vehicle, vehicle,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    original_filename, spr_object_key, fingerprint, steering,
    brake_help_enabled, automatic_gears, manual_shifter, axis_clutch,
    automatic_clutch, driver_side, abs_enabled, game_version, hlvc_status,
    created_at
FROM hotlap
WHERE source = 'upload';

ALTER TABLE hotlap
    ADD COLUMN submission_id BIGINT UNIQUE
        REFERENCES hotlap_submission (id) ON DELETE CASCADE;

UPDATE hotlap
SET submission_id = hotlap_submission.id
FROM hotlap_submission
WHERE hotlap.source = 'upload'
  AND hotlap.fingerprint = hotlap_submission.fingerprint;

-- Only already-valid uploaded rows remain leaderboard facts. All other states
-- now live solely in hotlap_submission.
DELETE FROM hotlap
WHERE source = 'upload'
  AND hlvc_status <> 'valid';

DROP INDEX hotlap_hlvc_pending_idx;
DROP INDEX hotlap_valid_era_chart_best_idx;

ALTER TABLE hotlap
    DROP COLUMN hlvc_status;

CREATE INDEX hotlap_era_chart_best_idx
    ON hotlap (era_id, track, vehicle, player_id, lap_time_ms, id);
