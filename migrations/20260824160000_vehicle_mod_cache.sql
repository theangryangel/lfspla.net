CREATE TABLE vehicle_mod (
    id TEXT PRIMARY KEY CHECK (id ~ '^[0-9A-F]{6}$'),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    normalized_name TEXT NOT NULL CHECK (btrim(normalized_name) <> ''),
    version SMALLINT NOT NULL CHECK (version BETWEEN 0 AND 999),
    class SMALLINT CHECK (class BETWEEN 0 AND 14),
    author_username TEXT,
    work_in_progress BOOLEAN NOT NULL DEFAULT FALSE,
    published_at TIMESTAMPTZ,
    available BOOLEAN NOT NULL DEFAULT TRUE,
    metadata JSONB NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX vehicle_mod_normalized_name_idx
    ON vehicle_mod (normalized_name, version, id);
