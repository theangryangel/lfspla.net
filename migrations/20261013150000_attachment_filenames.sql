-- Attachments own their identity and filename; identical bytes may share a path.
ALTER TABLE attachment
    ADD COLUMN original_filename TEXT,
    DROP CONSTRAINT attachment_object_key_key;
CREATE INDEX attachment_object_key_idx ON attachment (object_key);

-- Split existing shared records per reference, preserving the first UUID.
CREATE TEMP TABLE attachment_references ON COMMIT DROP AS
SELECT *, row_number() OVER (PARTITION BY attachment_id ORDER BY kind, parent_id) AS ordinal
FROM (
    SELECT spr_attachment_id AS attachment_id, 'spr' AS kind, id::text AS parent_id, original_filename
    FROM hotlap WHERE spr_attachment_id IS NOT NULL
    UNION ALL
    SELECT raf_attachment_id, 'raf', id::text, NULL::text
    FROM hotlap WHERE raf_attachment_id IS NOT NULL
    UNION ALL
    SELECT image_attachment_id, 'image', id, NULL::text
    FROM vehicle WHERE image_attachment_id IS NOT NULL
) refs;

DO $$
DECLARE
    ref RECORD;
    target_id BIGINT;
BEGIN
    FOR ref IN SELECT * FROM attachment_references ORDER BY attachment_id, ordinal LOOP
        IF ref.ordinal = 1 THEN
            target_id := ref.attachment_id;
            UPDATE attachment SET original_filename = ref.original_filename WHERE id = target_id;
        ELSE
            INSERT INTO attachment (object_key, sha256, size_bytes, media_type, extension, created_at, original_filename)
            SELECT object_key, sha256, size_bytes, media_type, extension, created_at, ref.original_filename
            FROM attachment WHERE id = ref.attachment_id
            RETURNING id INTO target_id;
        END IF;
        CASE ref.kind
            WHEN 'spr' THEN UPDATE hotlap SET spr_attachment_id = target_id WHERE id = ref.parent_id::bigint;
            WHEN 'raf' THEN UPDATE hotlap SET raf_attachment_id = target_id WHERE id = ref.parent_id::bigint;
            WHEN 'image' THEN UPDATE vehicle SET image_attachment_id = target_id WHERE id = ref.parent_id;
        END CASE;
    END LOOP;
END $$;

ALTER TABLE hotlap
    DROP CONSTRAINT hotlap_lifecycle_source_check,
    DROP CONSTRAINT hotlap_replay_provenance_check,
    DROP COLUMN original_filename,
    ADD CONSTRAINT hotlap_lifecycle_source_check CHECK (
        source = 'upload'
        OR (source IN ('lfsworld_v1', 'demo') AND state = 'valid'
            AND hlvc_result_code IS NULL AND attempt_count = 0
            AND next_attempt_at IS NULL AND started_at IS NULL AND finished_at IS NULL)
    ),
    ADD CONSTRAINT hotlap_replay_provenance_check CHECK (
        (source = 'upload' AND spr_attachment_id IS NOT NULL AND fingerprint ~ '^[0-9a-f]{64}$')
        OR (source = 'lfsworld_v1' AND spr_attachment_id IS NULL AND fingerprint ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}:[1-9][0-9]*$')
        OR (source = 'demo' AND spr_attachment_id IS NULL AND fingerprint LIKE 'demo:%')
    );
