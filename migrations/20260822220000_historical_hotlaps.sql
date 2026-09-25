ALTER TABLE player
    ALTER COLUMN last_authenticated_at DROP NOT NULL,
    ADD COLUMN lfsworld_id BIGINT UNIQUE;

ALTER TABLE hotlap
    ALTER COLUMN spr_object_key DROP NOT NULL,
    ADD COLUMN source TEXT NOT NULL DEFAULT 'upload',
    ADD COLUMN fingerprint TEXT;

UPDATE hotlap
SET fingerprint = spr_sha256;

ALTER TABLE hotlap
    DROP CONSTRAINT hotlap_spr_sha256_format,
    DROP CONSTRAINT hotlap_spr_sha256_unique,
    DROP COLUMN spr_sha256,
    ALTER COLUMN fingerprint SET NOT NULL,
    ADD CONSTRAINT hotlap_source_check CHECK (
        source IN ('upload', 'lfsworld_v1')
    ),
    ADD CONSTRAINT hotlap_fingerprint_nonempty CHECK (
        btrim(fingerprint) <> ''
    ),
    ADD CONSTRAINT hotlap_replay_provenance_check CHECK (
        (
            source = 'upload'
            AND spr_object_key IS NOT NULL
            AND fingerprint ~ '^[0-9a-f]{64}$'
        ) OR (
            source = 'lfsworld_v1'
            AND spr_object_key IS NULL
        )
    ),
    ADD CONSTRAINT hotlap_source_fingerprint_unique UNIQUE (source, fingerprint);
