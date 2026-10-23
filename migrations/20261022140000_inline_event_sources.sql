-- Source is TEXT on disk and a Rust enum in application models.
ALTER TABLE league
    ADD COLUMN source TEXT NOT NULL DEFAULT 'local' CHECK (source IN ('local', 'lfs_net')),
    ADD COLUMN external_id TEXT,
    ADD COLUMN source_url TEXT,
    ADD COLUMN last_seen_at TIMESTAMPTZ,
    ADD COLUMN last_synced_at TIMESTAMPTZ;
ALTER TABLE season
    ADD COLUMN source TEXT NOT NULL DEFAULT 'local' CHECK (source IN ('local', 'lfs_net')),
    ADD COLUMN external_id TEXT,
    ADD COLUMN source_url TEXT,
    ADD COLUMN last_seen_at TIMESTAMPTZ,
    ADD COLUMN last_synced_at TIMESTAMPTZ;
ALTER TABLE event
    ADD COLUMN source TEXT NOT NULL DEFAULT 'local' CHECK (source IN ('local', 'lfs_net')),
    ADD COLUMN external_id TEXT,
    ADD COLUMN source_url TEXT,
    ADD COLUMN last_seen_at TIMESTAMPTZ,
    ADD COLUMN last_synced_at TIMESTAMPTZ;

UPDATE league SET source = mapping.source, external_id = mapping.external_id,
    source_url = mapping.source_url, last_seen_at = mapping.last_seen_at,
    last_synced_at = mapping.last_synced_at
FROM league_source mapping WHERE mapping.league_id = league.id;
UPDATE season SET source = mapping.source, external_id = mapping.external_id,
    source_url = mapping.source_url, last_seen_at = mapping.last_seen_at,
    last_synced_at = mapping.last_synced_at
FROM season_source mapping WHERE mapping.season_id = season.id;
UPDATE event SET source = mapping.source, external_id = mapping.external_id,
    source_url = mapping.source_url, last_seen_at = mapping.last_seen_at,
    last_synced_at = mapping.last_synced_at
FROM event_source mapping WHERE mapping.event_id = event.id;

ALTER TABLE league
    ADD UNIQUE (source, external_id),
    ADD CHECK ((source = 'local' AND external_id IS NULL AND source_url IS NULL
                AND last_seen_at IS NULL AND last_synced_at IS NULL)
        OR (source = 'lfs_net' AND external_id IS NOT NULL AND length(trim(external_id)) > 0));
ALTER TABLE season
    ADD UNIQUE (source, external_id),
    ADD CHECK ((source = 'local' AND external_id IS NULL AND source_url IS NULL
                AND last_seen_at IS NULL AND last_synced_at IS NULL)
        OR (source = 'lfs_net' AND external_id IS NOT NULL AND length(trim(external_id)) > 0));
ALTER TABLE event
    ADD UNIQUE (source, external_id),
    ADD CHECK ((source = 'local' AND external_id IS NULL AND source_url IS NULL
                AND last_seen_at IS NULL AND last_synced_at IS NULL)
        OR (source = 'lfs_net' AND external_id IS NOT NULL AND length(trim(external_id)) > 0));

DROP TABLE event_source;
DROP TABLE season_source;
DROP TABLE league_source;
