CREATE TABLE team (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_id BIGINT NOT NULL REFERENCES player(id),
    name TEXT NOT NULL,
    description TEXT,
    website_url TEXT,
    discord_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX team_owner_id_idx ON team (owner_id);

CREATE TABLE team_member (
    team_id BIGINT NOT NULL REFERENCES team(id) ON DELETE CASCADE,
    player_id BIGINT NOT NULL REFERENCES player(id),
    role TEXT NOT NULL DEFAULT 'member',
    joined_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (team_id, player_id)
);

CREATE INDEX team_member_player_id_idx ON team_member (player_id);
