ALTER TABLE hotlap
    ADD CONSTRAINT hotlap_submission_provenance_check CHECK (
        (source = 'upload' AND submission_id IS NOT NULL)
        OR (source = 'lfsworld_v1' AND submission_id IS NULL)
    );
