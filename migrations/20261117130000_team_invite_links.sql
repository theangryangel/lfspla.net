ALTER TABLE team ADD COLUMN invite_token_hash BYTEA;
CREATE UNIQUE INDEX team_invite_token_hash_uidx ON team (invite_token_hash)
    WHERE invite_token_hash IS NOT NULL;
