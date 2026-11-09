-- Unknown durations remain point events; known durations use [start, end).
ALTER TABLE event
    ADD COLUMN ends_at TIMESTAMPTZ,
    ADD CONSTRAINT event_end_after_start CHECK (ends_at IS NULL OR ends_at > starts_at);

CREATE INDEX event_visible_end ON event (ends_at) WHERE NOT hidden AND ends_at IS NOT NULL;
