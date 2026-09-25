-- Existing images may already be stale after a failed refresh. Leave their
-- revision unknown so the next catalogue sync downloads them again.
ALTER TABLE vehicle ADD COLUMN image_version SMALLINT;
