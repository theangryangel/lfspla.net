CREATE TABLE webhook (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    player_id BIGINT NOT NULL REFERENCES player(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    url TEXT NOT NULL,
    format TEXT NOT NULL CHECK (format IN ('discord')),
    event_kind TEXT NOT NULL CHECK (event_kind IN ('hotlap_validated')),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (player_id, url, event_kind)
);
CREATE INDEX webhook_player ON webhook (player_id);
CREATE TABLE webhook_notification (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    webhook_id BIGINT NOT NULL REFERENCES webhook(id) ON DELETE CASCADE,
    hotlap_id BIGINT NOT NULL REFERENCES hotlap(id) ON DELETE CASCADE,
    event_kind TEXT NOT NULL CHECK (event_kind IN ('hotlap_validated')),
    event JSONB NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    error_detail TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (webhook_id, hotlap_id, event_kind)
);
CREATE INDEX webhook_notification_due ON webhook_notification (next_attempt_at, id)
    WHERE delivered_at IS NULL AND failed_at IS NULL;
CREATE INDEX webhook_notification_hotlap ON webhook_notification (hotlap_id);
