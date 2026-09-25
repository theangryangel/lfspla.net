CREATE TABLE personal_access_token (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    player_id BIGINT NOT NULL REFERENCES player (id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (
        btrim(name) <> '' AND char_length(name) <= 100
    ),
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    token_hint TEXT NOT NULL CHECK (
        token_hint ~ '^lfspla_…[0-9a-f]{8}$'
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL CHECK (expires_at > created_at),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX personal_access_token_player_created_idx
    ON personal_access_token (player_id, created_at DESC);

