-- Land Speed Record uses the scheduled game lifecycle and its own classification.
ALTER TABLE event DROP CONSTRAINT event_source_check;
ALTER TABLE event ADD CONSTRAINT event_source_check CHECK (source IN ('local','lfs_net','bomb','metronome','shortcut','land_speed_record'));
ALTER TABLE event DROP CONSTRAINT event_provenance;
ALTER TABLE event
ADD CONSTRAINT event_provenance CHECK (
      (source = 'lfs_net' AND external_id IS NOT NULL AND length(trim(external_id)) > 0
        AND catalogue_id IS NULL AND game_config IS NULL AND layout_attachment_id IS NULL)
      OR (source = 'local' AND external_id IS NULL AND source_url IS NULL
        AND last_seen_at IS NULL AND last_synced_at IS NULL
        AND game_config IS NULL AND layout_attachment_id IS NULL)
      OR (source IN ('bomb','metronome','shortcut','land_speed_record') AND catalogue_id IS NOT NULL
        AND length(trim(catalogue_id)) > 0 AND game_config IS NOT NULL
        AND jsonb_typeof(game_config) = 'object'
        AND game_config->'source'->>'type' IS NOT NULL
        AND game_config->'source'->>'type' = source
        AND layout_attachment_id IS NOT NULL AND ends_at IS NOT NULL
        AND external_id IS NULL AND source_url IS NULL
        AND last_seen_at IS NULL AND last_synced_at IS NULL)
    );
ALTER TABLE event DROP CONSTRAINT game_events_do_not_overlap;
ALTER TABLE event ADD CONSTRAINT game_events_do_not_overlap EXCLUDE USING gist
    (tstzrange(starts_at, ends_at, '[)') WITH &&)
    WHERE (source IN ('bomb','metronome','shortcut','land_speed_record') AND NOT hidden)
    DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE land_speed_record_run (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id BIGINT NOT NULL REFERENCES event(id),
    player_id BIGINT NOT NULL REFERENCES player(id),
    vehicle TEXT NOT NULL,
    speed_kph DOUBLE PRECISION NOT NULL CHECK (speed_kph > 0 AND speed_kph < 720),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX land_speed_record_run_event ON land_speed_record_run(event_id, player_id);
CREATE VIEW classified_land_speed_record_run AS
SELECT best.*, row_number() OVER (PARTITION BY event_id ORDER BY speed_kph DESC, recorded_at, id) AS position
FROM (SELECT DISTINCT ON (event_id, player_id) * FROM land_speed_record_run
      ORDER BY event_id, player_id, speed_kph DESC, recorded_at, id) best;

CREATE OR REPLACE VIEW game_event_standing AS
SELECT event_id, position, player_id, vehicle, checkpoint_count, survival_ms, NULL::bigint AS time_ms, NULL::bigint AS delta_ms, NULL::double precision AS speed_kph FROM classified_bomb_run
UNION ALL
SELECT event_id, position, player_id, vehicle, NULL::bigint, NULL::bigint, time_ms, NULL::bigint, NULL::double precision FROM classified_shortcut_time
UNION ALL
SELECT event_id, position, player_id, NULL::text, NULL::bigint, NULL::bigint, NULL::bigint, delta_ms, NULL::double precision FROM classified_metronome_result
UNION ALL
SELECT event_id, position, player_id, vehicle, NULL::bigint, NULL::bigint, NULL::bigint, NULL::bigint, speed_kph FROM classified_land_speed_record_run;
