CREATE TABLE player (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    lfs_username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    country_code TEXT CHECK (country_code IS NULL OR country_code ~ '^[A-Z]{2}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_authenticated_at TIMESTAMPTZ NOT NULL
);

CREATE SCHEMA tower_sessions;

CREATE TABLE tower_sessions.session (
    id TEXT PRIMARY KEY,
    data BYTEA NOT NULL,
    expiry_date TIMESTAMPTZ NOT NULL
);

CREATE INDEX tower_sessions_expiry_date_idx
    ON tower_sessions.session (expiry_date);
