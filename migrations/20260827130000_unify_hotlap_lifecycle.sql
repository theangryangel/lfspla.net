-- A hotlap is the uploaded/imported lap throughout its lifecycle. Public
-- queries expose only rows whose state is `valid`; there is no second
-- publication row and no submission-to-hotlap identity transition.
ALTER TABLE hotlap
    DROP CONSTRAINT hotlap_submission_provenance_check,
    ALTER COLUMN vehicle DROP NOT NULL,
    ADD COLUMN raw_vehicle_name TEXT,
    ADD COLUMN mod_version SMALLINT CHECK (mod_version BETWEEN 0 AND 999),
    ADD COLUMN state TEXT NOT NULL DEFAULT 'valid' CHECK (
        state IN (
            'pending', 'validating', 'awaiting_vehicle',
            'valid', 'invalid', 'error'
        )
    ),
    ADD COLUMN hlvc_result_code SMALLINT CHECK (
        hlvc_result_code BETWEEN 0 AND 21
    ),
    ADD COLUMN error_detail TEXT,
    ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (
        attempt_count >= 0
    ),
    ADD COLUMN next_attempt_at TIMESTAMPTZ,
    ADD COLUMN started_at TIMESTAMPTZ,
    ADD COLUMN finished_at TIMESTAMPTZ;

-- Every existing hotlap is already public. Historical imports have no local
-- HLVC evidence; existing uploaded rows do, even when the old submission row
-- predates explicit attempt/result timestamps.
UPDATE hotlap
SET raw_vehicle_name = vehicle,
    state = 'valid',
    hlvc_result_code = CASE WHEN source = 'upload' THEN 1 ELSE NULL END,
    finished_at = CASE WHEN source = 'upload' THEN created_at ELSE NULL END;

-- Preserve the richer upload lifecycle on already-published rows without
-- changing their public hotlap IDs.
UPDATE hotlap
SET raw_vehicle_name = submission.raw_vehicle_name,
    mod_version = submission.mod_version,
    state = CASE
        WHEN submission.vehicle IS NULL THEN 'awaiting_vehicle'
        ELSE submission.hlvc_status
    END,
    hlvc_result_code = CASE
        WHEN submission.vehicle IS NULL THEN NULL
        WHEN submission.hlvc_status = 'valid'
            THEN COALESCE(submission.hlvc_result_code, 1)
        ELSE submission.hlvc_result_code
    END,
    error_detail = CASE
        WHEN submission.vehicle IS NULL THEN NULL
        ELSE submission.error_detail
    END,
    attempt_count = CASE
        WHEN submission.vehicle IS NULL THEN 0
        ELSE submission.attempt_count
    END,
    next_attempt_at = CASE
        WHEN submission.vehicle IS NULL THEN NULL
        ELSE submission.next_attempt_at
    END,
    started_at = CASE
        WHEN submission.vehicle IS NULL THEN NULL
        ELSE submission.started_at
    END,
    finished_at = CASE
        WHEN submission.vehicle IS NULL THEN NULL
        WHEN submission.hlvc_status = 'valid'
            THEN COALESCE(submission.finished_at, hotlap.created_at)
        ELSE submission.finished_at
    END
FROM hotlap_submission AS submission
WHERE hotlap.submission_id = submission.id;

-- Pending, invalid, failed, and unresolved uploads had no hotlap identity in
-- the split model. They now become ordinary lifecycle rows.
INSERT INTO hotlap (
    player_id, era_id, track, vehicle, raw_vehicle_name, mod_version,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    original_filename, spr_object_key, source, fingerprint, steering,
    brake_help_enabled, automatic_gears, manual_shifter, axis_clutch,
    automatic_clutch, driver_side, abs_enabled, created_at, game_version,
    state, hlvc_result_code, error_detail, attempt_count, next_attempt_at,
    started_at, finished_at
)
SELECT
    submission.player_id,
    submission.era_id,
    submission.track,
    submission.vehicle,
    submission.raw_vehicle_name,
    submission.mod_version,
    submission.lap_time_ms,
    submission.split_1_ms,
    submission.split_2_ms,
    submission.split_3_ms,
    submission.split_4_ms,
    submission.original_filename,
    submission.spr_object_key,
    'upload',
    submission.fingerprint,
    submission.steering,
    submission.brake_help_enabled,
    submission.automatic_gears,
    submission.manual_shifter,
    submission.axis_clutch,
    submission.automatic_clutch,
    submission.driver_side,
    submission.abs_enabled,
    submission.created_at,
    submission.game_version,
    CASE
        WHEN submission.vehicle IS NULL THEN 'awaiting_vehicle'
        ELSE submission.hlvc_status
    END,
    CASE
        WHEN submission.vehicle IS NULL THEN NULL
        WHEN submission.hlvc_status = 'valid'
            THEN COALESCE(submission.hlvc_result_code, 1)
        ELSE submission.hlvc_result_code
    END,
    CASE WHEN submission.vehicle IS NULL THEN NULL ELSE submission.error_detail END,
    CASE WHEN submission.vehicle IS NULL THEN 0 ELSE submission.attempt_count END,
    CASE WHEN submission.vehicle IS NULL THEN NULL ELSE submission.next_attempt_at END,
    CASE WHEN submission.vehicle IS NULL THEN NULL ELSE submission.started_at END,
    CASE
        WHEN submission.vehicle IS NULL THEN NULL
        WHEN submission.hlvc_status = 'valid'
            THEN COALESCE(submission.finished_at, submission.created_at)
        ELSE submission.finished_at
    END
FROM hotlap_submission AS submission
LEFT JOIN hotlap AS published ON published.submission_id = submission.id
WHERE published.id IS NULL;

ALTER TABLE hotlap
    DROP COLUMN submission_id,
    ALTER COLUMN raw_vehicle_name SET NOT NULL,
    -- No default: every writer must make the lifecycle decision explicitly.
    ALTER COLUMN state DROP DEFAULT,
    ADD CONSTRAINT hotlap_raw_vehicle_name_nonempty CHECK (
        btrim(raw_vehicle_name) <> ''
    ),
    ADD CONSTRAINT hotlap_vehicle_state_check CHECK (
        (
            state = 'awaiting_vehicle'
            AND source = 'upload'
            AND vehicle IS NULL
            AND hlvc_result_code IS NULL
            AND attempt_count = 0
            AND next_attempt_at IS NULL
            AND started_at IS NULL
            AND finished_at IS NULL
        ) OR (
            state <> 'awaiting_vehicle'
            AND vehicle IS NOT NULL
        )
    ),
    ADD CONSTRAINT hotlap_valid_state_check CHECK (
        state <> 'valid'
        OR (
            vehicle IS NOT NULL
            AND (
                source = 'lfsworld_v1'
                OR (hlvc_result_code = 1 AND finished_at IS NOT NULL)
            )
        )
    ),
    ADD CONSTRAINT hotlap_lifecycle_source_check CHECK (
        (source = 'upload' AND original_filename IS NOT NULL)
        OR (
            source = 'lfsworld_v1'
            AND state = 'valid'
            AND hlvc_result_code IS NULL
            AND attempt_count = 0
            AND next_attempt_at IS NULL
            AND started_at IS NULL
            AND finished_at IS NULL
        )
    );

DROP TABLE hotlap_submission;

DROP INDEX hotlap_era_chart_best_idx;

CREATE INDEX hotlap_player_outstanding_idx
    ON hotlap (player_id)
    WHERE source = 'upload'
      AND state IN ('pending', 'validating', 'awaiting_vehicle', 'error');

CREATE INDEX hotlap_validation_queue_idx
    ON hotlap (next_attempt_at, created_at)
    WHERE source = 'upload'
      AND state IN ('pending', 'error');

CREATE INDEX hotlap_recent_valid_idx
    ON hotlap (finished_at DESC, id DESC)
    WHERE source = 'upload'
      AND state = 'valid'
      AND finished_at IS NOT NULL;

CREATE INDEX hotlap_valid_era_chart_best_idx
    ON hotlap (era_id, track, vehicle, player_id, lap_time_ms, id)
    WHERE state = 'valid';
