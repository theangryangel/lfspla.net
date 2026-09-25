ALTER TABLE hotlap
    ADD COLUMN original_filename TEXT,
    ADD COLUMN spr_sha256 TEXT;

-- Existing object keys embed the digest as `spr/xx/<sha256>.spr`.
UPDATE hotlap
SET spr_sha256 = split_part(split_part(spr_object_key, '/', 3), '.', 1);

ALTER TABLE hotlap
    ALTER COLUMN spr_sha256 SET NOT NULL,
    ADD CONSTRAINT hotlap_original_filename_nonempty CHECK (
        original_filename IS NULL OR btrim(original_filename) <> ''
    ),
    ADD CONSTRAINT hotlap_spr_sha256_format CHECK (
        spr_sha256 ~ '^[0-9a-f]{64}$'
    ),
    ADD CONSTRAINT hotlap_spr_sha256_unique UNIQUE (spr_sha256);
