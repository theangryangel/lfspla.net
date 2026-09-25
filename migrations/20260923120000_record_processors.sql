-- Stop the old validator before this cutover. Object GC handles deleted replays.
DELETE FROM hotlap WHERE vehicle IS NULL;

UPDATE hotlap
SET state = 'pending'
WHERE state NOT IN ('pending', 'valid', 'invalid');

-- Give all outstanding records a fresh retry budget under the new processor.
UPDATE hotlap
SET attempt_count = 0, next_attempt_at = NULL, started_at = NULL,
    finished_at = NULL, hlvc_result_code = NULL, error_detail = NULL
WHERE state = 'pending';

ALTER TABLE hotlap
    DROP CONSTRAINT hotlap_state_check,
    DROP CONSTRAINT hotlap_vehicle_state_check,
    ALTER COLUMN vehicle SET NOT NULL,
    ADD CONSTRAINT hotlap_state_check CHECK (state IN ('pending', 'valid', 'invalid'));

DROP INDEX hotlap_player_outstanding_idx;
CREATE INDEX hotlap_player_outstanding_idx ON hotlap (player_id)
    WHERE source = 'upload' AND state = 'pending';
DROP INDEX hotlap_validation_queue_idx;
CREATE INDEX hotlap_validation_queue_idx ON hotlap (next_attempt_at, created_at, id)
    WHERE source = 'upload' AND state = 'pending';
