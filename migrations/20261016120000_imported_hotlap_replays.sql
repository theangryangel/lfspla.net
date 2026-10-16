-- Historical CSV imports may optionally have their original SPR attached.
-- Their era/item identity and published lifecycle remain unchanged.
ALTER TABLE hotlap
    DROP CONSTRAINT hotlap_replay_provenance_check,
    ADD CONSTRAINT hotlap_replay_provenance_check CHECK (
        (source = 'upload' AND spr_attachment_id IS NOT NULL AND fingerprint ~ '^[0-9a-f]{64}$')
        OR (source = 'lfsworld_v1' AND fingerprint ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}:[1-9][0-9]*$')
        OR (source = 'demo' AND spr_attachment_id IS NULL AND fingerprint LIKE 'demo:%')
    );
