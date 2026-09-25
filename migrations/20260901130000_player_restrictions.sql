-- Account restrictions are independent: denying authentication must not be
-- required to deny uploads, and denying uploads must not revoke authentication
-- or prevent a player from managing existing hotlaps.
ALTER TABLE player
    ADD COLUMN deny_auth BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN deny_uploads BOOLEAN NOT NULL DEFAULT FALSE;
