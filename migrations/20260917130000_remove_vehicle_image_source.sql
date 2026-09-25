ALTER TABLE vehicle
    DROP CONSTRAINT vehicle_image_metadata_check,
    DROP COLUMN image_source_url,
    ADD CONSTRAINT vehicle_image_metadata_check CHECK (
        (image_object_key IS NULL AND image_content_type IS NULL AND image_fetched_at IS NULL)
        OR (image_object_key IS NOT NULL AND image_content_type IS NOT NULL AND image_fetched_at IS NOT NULL)
    );
