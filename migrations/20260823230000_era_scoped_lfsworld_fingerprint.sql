ALTER TABLE hotlap
    DROP CONSTRAINT hotlap_replay_provenance_check,
    DROP CONSTRAINT hotlap_source_fingerprint_unique;

-- LFSWorld restarted its item numbers whenever an incompatible hotlap table
-- was archived. Include the era in the external identity so all snapshots can
-- coexist and remain idempotent when imported again.
UPDATE hotlap
SET fingerprint = era_id || ':' || fingerprint
WHERE source = 'lfsworld_v1'
  AND fingerprint ~ '^[1-9][0-9]*$';

ALTER TABLE hotlap
    ADD CONSTRAINT hotlap_replay_provenance_check CHECK (
        (
            source = 'upload'
            AND spr_object_key IS NOT NULL
            AND fingerprint ~ '^[0-9a-f]{64}$'
        ) OR (
            source = 'lfsworld_v1'
            AND spr_object_key IS NULL
            AND fingerprint ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}:[1-9][0-9]*$'
        )
    ),
    ADD CONSTRAINT hotlap_source_fingerprint_unique UNIQUE (source, fingerprint);
