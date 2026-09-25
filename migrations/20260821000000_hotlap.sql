CREATE TABLE hotlap (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    player_id BIGINT NOT NULL REFERENCES player (id),
    era_id TEXT NOT NULL,
    track TEXT NOT NULL,
    vehicle TEXT NOT NULL,
    split_1_ms BIGINT NOT NULL CHECK (split_1_ms >= 0),
    split_2_ms BIGINT NOT NULL CHECK (split_2_ms >= 0),
    split_3_ms BIGINT NOT NULL CHECK (split_3_ms >= 0),
    split_4_ms BIGINT NOT NULL CHECK (split_4_ms >= 0),
    spr_object_key TEXT NOT NULL,
    flags INTEGER NOT NULL,
    steering TEXT NOT NULL CHECK (
        steering IN ('wheel', 'mouse', 'keyboard', 'keyboard_stabilised')
    ),
    abs_enabled BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    game_version TEXT NOT NULL,
    hlvc_status TEXT NOT NULL DEFAULT 'pending' CHECK (
        hlvc_status IN ('pending', 'validating', 'valid', 'invalid', 'error')
    )
);

CREATE INDEX hotlap_player_created_idx ON hotlap (player_id, created_at DESC);
CREATE INDEX hotlap_chart_idx ON hotlap (era_id, track, vehicle, hlvc_status);
CREATE INDEX hotlap_hlvc_pending_idx ON hotlap (created_at)
    WHERE hlvc_status = 'pending';
