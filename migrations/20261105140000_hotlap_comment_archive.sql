-- Stable conversation lookup, including after the hotlap reference is cleared.
CREATE INDEX hotlap_comment_original_lap ON hotlap_comment (original_hotlap_id, parent_id, id);
