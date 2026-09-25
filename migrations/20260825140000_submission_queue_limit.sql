CREATE INDEX hotlap_submission_player_outstanding_idx
    ON hotlap_submission (player_id)
    WHERE hlvc_status IN ('pending', 'validating', 'error');
