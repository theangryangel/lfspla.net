CREATE INDEX hotlap_submission_recent_valid_idx
    ON hotlap_submission (finished_at DESC, id DESC)
    WHERE hlvc_status = 'valid' AND finished_at IS NOT NULL;
