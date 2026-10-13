ALTER TABLE hotlap
    ADD COLUMN raf_object_key TEXT,
    ADD CONSTRAINT hotlap_raf_object_key_nonempty CHECK (
        raf_object_key IS NULL OR btrim(raf_object_key) <> ''
    );
