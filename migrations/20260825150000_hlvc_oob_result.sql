ALTER TABLE hotlap_submission
    DROP CONSTRAINT hotlap_submission_hlvc_result_code_check;

ALTER TABLE hotlap_submission
    ADD CONSTRAINT hotlap_submission_hlvc_result_code_check
        CHECK (hlvc_result_code BETWEEN 0 AND 21);
