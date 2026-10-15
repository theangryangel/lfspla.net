-- Track artwork uses the shared attachment store. Existing catalogue rows and
-- their chart/hotlap identities are retained.
ALTER TABLE track
    ADD COLUMN outline_attachment_id BIGINT REFERENCES attachment(id),
    ADD COLUMN image_attachment_id BIGINT REFERENCES attachment(id);
CREATE INDEX track_outline_attachment_idx ON track(outline_attachment_id);
CREATE INDEX track_image_attachment_idx ON track(image_attachment_id);
