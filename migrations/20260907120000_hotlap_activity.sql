-- Public activity includes imported and uploaded validated laps, ordered by
-- submission time. The existing recent-valid index is upload/finish-time only.
CREATE INDEX hotlap_activity_valid_idx
    ON hotlap (created_at DESC, id DESC)
    WHERE state = 'valid';
