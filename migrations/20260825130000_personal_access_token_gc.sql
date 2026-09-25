CREATE INDEX personal_access_token_expires_idx
    ON personal_access_token (expires_at);

CREATE INDEX personal_access_token_revoked_idx
    ON personal_access_token (revoked_at)
    WHERE revoked_at IS NOT NULL;
