-- Preserve applied migration checksums while standardising displayed token hints.
ALTER TABLE personal_access_token
    DROP CONSTRAINT personal_access_token_token_hint_check;

UPDATE personal_access_token
SET token_hint = replace(token_hint, chr(8230), '...')
WHERE strpos(token_hint, chr(8230)) > 0;

ALTER TABLE personal_access_token
    ADD CONSTRAINT personal_access_token_token_hint_check
    CHECK (token_hint ~ '^lfspla_[.]{3}[0-9a-f]{8}$');
