CREATE TABLE community_card (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    sequence INTEGER NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    sponsored BOOLEAN NOT NULL DEFAULT FALSE,
    theme TEXT NOT NULL DEFAULT 'neutral'
        CHECK (theme IN ('neutral', 'cyan', 'purple', 'yellow')),
    layout TEXT NOT NULL DEFAULT 'standard'
        CHECK (layout IN ('standard', 'split')),
    content JSONB NOT NULL CHECK (
        jsonb_typeof(content) = 'object'
        AND content ? 'type'
        AND content->>'type' IN ('links', 'partner', 'image', 'announcement')
    ),
    image_attachment_id BIGINT REFERENCES attachment(id)
);
