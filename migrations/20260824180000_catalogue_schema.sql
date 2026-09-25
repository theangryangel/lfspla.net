CREATE TABLE era (
    id TEXT PRIMARY KEY CHECK (id ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}$'),
    title TEXT NOT NULL CHECK (btrim(title) <> ''),
    version_requirement TEXT NOT NULL CHECK (btrim(version_requirement) <> ''),
    open BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE UNIQUE INDEX era_single_open_idx ON era (open) WHERE open;

CREATE TABLE vehicle (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('standard', 'mod')),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    normalized_name TEXT NOT NULL CHECK (btrim(normalized_name) <> ''),
    license TEXT NOT NULL CHECK (license IN ('demo', 's1', 's2', 's3')),
    version SMALLINT CHECK (version BETWEEN 0 AND 999),
    class SMALLINT CHECK (class BETWEEN 0 AND 14),
    author_username TEXT,
    work_in_progress BOOLEAN NOT NULL DEFAULT FALSE,
    published_at TIMESTAMPTZ,
    available BOOLEAN NOT NULL DEFAULT TRUE,
    metadata JSONB,
    fetched_at TIMESTAMPTZ,
    last_seen_at TIMESTAMPTZ,
    CONSTRAINT vehicle_id_kind_check CHECK (
        (kind = 'standard' AND id ~ '^[A-Z][A-Z0-9]{2}$')
        OR (kind = 'mod' AND id ~ '^[0-9A-F]{6}$')
    )
);

CREATE INDEX vehicle_normalized_name_idx
    ON vehicle (normalized_name, version, id);

CREATE INDEX vehicle_available_name_idx
    ON vehicle (kind, name, id)
    WHERE available;

-- Retain every observed mod before the old cache is retired. This copies
-- mutable upstream metadata only; it deliberately creates no era policy.
INSERT INTO vehicle (
    id, kind, name, normalized_name, license, version, class,
    author_username, work_in_progress, published_at, available, metadata,
    fetched_at, last_seen_at
)
SELECT
    id, 'mod', name, normalized_name, 's3', version, class,
    author_username, work_in_progress, published_at, available, metadata,
    fetched_at, last_seen_at
FROM vehicle_mod;

CREATE TABLE track (
    id TEXT PRIMARY KEY CHECK (btrim(id) <> ''),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    license TEXT NOT NULL CHECK (license IN ('demo', 's1', 's2', 's3', 'unknown')),
    reverse BOOLEAN NOT NULL,
    open_configuration BOOLEAN NOT NULL
);

CREATE TABLE era_track (
    era_id TEXT NOT NULL REFERENCES era (id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES track (id) ON DELETE RESTRICT,
    PRIMARY KEY (era_id, track_id)
);

CREATE TABLE era_vehicle (
    era_id TEXT NOT NULL REFERENCES era (id) ON DELETE CASCADE,
    vehicle_id TEXT NOT NULL REFERENCES vehicle (id) ON DELETE RESTRICT,
    PRIMARY KEY (era_id, vehicle_id)
);

CREATE TABLE ranking (
    era_id TEXT NOT NULL REFERENCES era (id) ON DELETE CASCADE,
    id TEXT NOT NULL CHECK (id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    position INTEGER NOT NULL CHECK (position >= 0),
    title TEXT NOT NULL CHECK (btrim(title) <> ''),
    description TEXT NOT NULL CHECK (btrim(description) <> ''),
    model TEXT NOT NULL CHECK (model IN ('lfs_world_v1')),
    badge JSONB CHECK (
        badge IS NULL OR (
            jsonb_typeof(badge) = 'object'
            AND badge ? 'label'
            AND jsonb_typeof(badge -> 'label') = 'string'
            AND badge ? 'qualification'
            AND jsonb_typeof(badge -> 'qualification') = 'object'
        )
    ),
    PRIMARY KEY (era_id, id),
    UNIQUE (era_id, position)
);

CREATE TABLE ranking_chart (
    era_id TEXT NOT NULL,
    ranking_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    track_id TEXT NOT NULL,
    vehicle_id TEXT NOT NULL,
    PRIMARY KEY (era_id, ranking_id, position),
    UNIQUE (era_id, ranking_id, track_id, vehicle_id),
    FOREIGN KEY (era_id, ranking_id)
        REFERENCES ranking (era_id, id) ON DELETE CASCADE,
    FOREIGN KEY (era_id, track_id)
        REFERENCES era_track (era_id, track_id) ON DELETE RESTRICT,
    FOREIGN KEY (era_id, vehicle_id)
        REFERENCES era_vehicle (era_id, vehicle_id) ON DELETE RESTRICT
);

CREATE TABLE player_era_badges (
    era_id TEXT NOT NULL REFERENCES era (id) ON DELETE CASCADE,
    player_id BIGINT NOT NULL REFERENCES player (id) ON DELETE CASCADE,
    badges JSONB NOT NULL CHECK (
        jsonb_typeof(badges) = 'array'
        AND jsonb_array_length(badges) > 0
    ),
    PRIMARY KEY (era_id, player_id)
);

CREATE INDEX player_era_badges_player_idx
    ON player_era_badges (player_id, era_id);
