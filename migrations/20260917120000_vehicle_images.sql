ALTER TABLE vehicle
    ADD COLUMN image_object_key TEXT,
    ADD COLUMN image_content_type TEXT,
    ADD COLUMN image_source_url TEXT,
    ADD COLUMN image_fetched_at TIMESTAMPTZ,
    ADD CONSTRAINT vehicle_image_metadata_check CHECK (
        (image_object_key IS NULL AND image_content_type IS NULL AND image_source_url IS NULL AND image_fetched_at IS NULL)
        OR (image_object_key IS NOT NULL AND image_content_type IS NOT NULL AND image_source_url IS NOT NULL AND image_fetched_at IS NOT NULL)
    );
