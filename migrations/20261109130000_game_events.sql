-- League/season provenance remains unchanged; game sources belong only to events.
ALTER TABLE event DROP CONSTRAINT event_source_check;
ALTER TABLE event ADD CONSTRAINT event_source_check CHECK (source IN ('local','lfs_net','bomb','metronome','shortcut'));
-- Replace the old local/upstream provenance invariant with an event-specific one.
DO $$ DECLARE constraint_name TEXT; BEGIN
  FOR constraint_name IN SELECT conname FROM pg_constraint
    WHERE conrelid = 'event'::regclass AND contype = 'c'
      AND pg_get_constraintdef(oid) LIKE '%last_synced_at%'
  LOOP EXECUTE format('ALTER TABLE event DROP CONSTRAINT %I', constraint_name); END LOOP;
END $$;
ALTER TABLE event
    ADD COLUMN catalogue_id TEXT UNIQUE,
    ADD COLUMN game_config JSONB,
    ADD COLUMN layout_attachment_id BIGINT REFERENCES attachment(id),
    ADD CONSTRAINT event_provenance CHECK (
      (source = 'lfs_net' AND external_id IS NOT NULL AND length(trim(external_id)) > 0
        AND catalogue_id IS NULL AND game_config IS NULL AND layout_attachment_id IS NULL)
      OR (source = 'local' AND external_id IS NULL AND source_url IS NULL
        AND last_seen_at IS NULL AND last_synced_at IS NULL
        AND game_config IS NULL AND layout_attachment_id IS NULL)
      OR (source IN ('bomb','metronome','shortcut') AND catalogue_id IS NOT NULL
        AND length(trim(catalogue_id)) > 0 AND game_config IS NOT NULL
        AND jsonb_typeof(game_config) = 'object'
        AND game_config->'source'->>'type' IS NOT NULL
        AND game_config->'source'->>'type' = source
        AND layout_attachment_id IS NOT NULL AND ends_at IS NOT NULL
        AND external_id IS NULL AND source_url IS NULL
        AND last_seen_at IS NULL AND last_synced_at IS NULL)
    );

-- A catalogue-wide check is also made in the seed transaction for useful errors.
-- This constraint additionally protects against concurrent/direct SQL writers.
ALTER TABLE event ADD CONSTRAINT game_events_do_not_overlap EXCLUDE USING gist
    (tstzrange(starts_at, ends_at, '[)') WITH &&)
    WHERE (source IN ('bomb','metronome','shortcut') AND NOT hidden)
    DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE game_event_execution (
    event_id BIGINT PRIMARY KEY REFERENCES event(id),
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    stopped_at TIMESTAMPTZ,
    last_error TEXT
);
CREATE TABLE bomb_run (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id BIGINT NOT NULL REFERENCES event(id),
    player_id BIGINT NOT NULL REFERENCES player(id),
    vehicle TEXT NOT NULL,
    checkpoint_count BIGINT NOT NULL CHECK (checkpoint_count >= 0),
    survival_ms BIGINT NOT NULL CHECK (survival_ms >= 0),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX bomb_run_event ON bomb_run(event_id, player_id);
CREATE TABLE shortcut_time (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id BIGINT NOT NULL REFERENCES event(id),
    player_id BIGINT NOT NULL REFERENCES player(id),
    vehicle TEXT NOT NULL,
    time_ms BIGINT NOT NULL CHECK (time_ms > 0),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX shortcut_time_event ON shortcut_time(event_id, player_id);
CREATE TABLE metronome_result (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id BIGINT NOT NULL REFERENCES event(id),
    player_id BIGINT NOT NULL REFERENCES player(id),
    delta_ms BIGINT NOT NULL CHECK (delta_ms > '-9223372036854775808'::BIGINT),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX metronome_result_event ON metronome_result(event_id, player_id);

CREATE VIEW classified_bomb_run AS
SELECT best.*, row_number() OVER (PARTITION BY event_id ORDER BY checkpoint_count DESC, survival_ms DESC, recorded_at, id) AS position
FROM (SELECT DISTINCT ON (event_id, player_id) * FROM bomb_run ORDER BY event_id, player_id, checkpoint_count DESC, survival_ms DESC, recorded_at, id) best;
CREATE VIEW classified_shortcut_time AS
SELECT best.*, row_number() OVER (PARTITION BY event_id ORDER BY time_ms, recorded_at, id) AS position
FROM (SELECT DISTINCT ON (event_id, player_id) * FROM shortcut_time ORDER BY event_id, player_id, time_ms, recorded_at, id) best;
CREATE VIEW classified_metronome_result AS
SELECT best.*, row_number() OVER (PARTITION BY event_id ORDER BY abs(delta_ms), recorded_at, id) AS position
FROM (SELECT DISTINCT ON (event_id, player_id) * FROM metronome_result ORDER BY event_id, player_id, abs(delta_ms), recorded_at, id) best;

-- A read projection only: each mode keeps its own storage and ordering rules.
CREATE VIEW game_event_standing AS
SELECT event_id, position, player_id, vehicle, checkpoint_count, survival_ms, NULL::bigint AS time_ms, NULL::bigint AS delta_ms FROM classified_bomb_run
UNION ALL
SELECT event_id, position, player_id, vehicle, NULL::bigint, NULL::bigint, time_ms, NULL::bigint FROM classified_shortcut_time
UNION ALL
SELECT event_id, position, player_id, NULL::text, NULL::bigint, NULL::bigint, NULL::bigint, delta_ms FROM classified_metronome_result;
