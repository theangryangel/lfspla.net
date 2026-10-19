-- Badge definitions are catalogue data; eligibility remains application code.
CREATE TABLE badge (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9][a-z0-9-]{0,79}$'),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    description TEXT NOT NULL CHECK (length(trim(description)) BETWEEN 1 AND 1000),
    sequence INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE badge_variant (
    badge_id TEXT NOT NULL REFERENCES badge(id) ON DELETE RESTRICT,
    id TEXT NOT NULL CHECK (id ~ '^[a-z0-9][a-z0-9-]{0,79}$'),
    image_attachment_id BIGINT REFERENCES attachment(id) ON DELETE RESTRICT,
    PRIMARY KEY (badge_id, id)
);

CREATE TABLE badge_award (
    id BIGSERIAL PRIMARY KEY,
    player_id BIGINT NOT NULL REFERENCES player(id) ON DELETE CASCADE,
    era_id BIGINT REFERENCES era(id) ON DELETE CASCADE,
    badge_id TEXT NOT NULL,
    variant TEXT NOT NULL DEFAULT 'standard',
    origin TEXT NOT NULL CHECK (origin IN ('automatic', 'manual')),
    source TEXT NOT NULL CHECK (length(trim(source)) > 0),
    detail TEXT CHECK (detail IS NULL OR length(trim(detail)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (badge_id, variant) REFERENCES badge_variant(badge_id, id) ON DELETE RESTRICT,
    CHECK (origin <> 'automatic' OR era_id IS NOT NULL),
    UNIQUE NULLS NOT DISTINCT (player_id, era_id, badge_id, origin, source)
);
CREATE INDEX badge_award_era_idx ON badge_award (era_id, player_id);

-- Keep the old projection available for inspection during the maintenance window.
-- No authoritative player or hotlap data is removed. Rebuild into badge_award after seeding.
ALTER TABLE player_era_badges RENAME TO player_era_badges_legacy;
INSERT INTO era_badge_refresh (era_id) SELECT id FROM era ON CONFLICT DO NOTHING;

-- Replace the legacy JSON shape check before changing stored configuration.
ALTER TABLE ranking DROP CONSTRAINT ranking_badge_check;
ALTER TABLE ranking ADD CONSTRAINT ranking_badge_check CHECK (
    badge IS NULL OR (jsonb_typeof(badge) = 'object'
        AND badge ? 'id' AND jsonb_typeof(badge -> 'id') = 'string'
        AND badge ? 'qualification' AND jsonb_typeof(badge -> 'qualification') = 'object')
) NOT VALID;

-- Existing ranking policies are retained, with labels replaced by catalogue references.
UPDATE ranking SET badge =
    (badge - 'label') || jsonb_build_object('id', slug) ||
    CASE WHEN badge ? 'completion' THEN
        jsonb_build_object('completion',
            ((badge -> 'completion') - 'label') || jsonb_build_object('id',
                CASE WHEN slug = 'nutter' THEN 'gabor' ELSE slug || '-completion' END))
    ELSE '{}'::jsonb END
WHERE badge IS NOT NULL;

ALTER TABLE ranking VALIDATE CONSTRAINT ranking_badge_check;

-- Bootstrap existing identities without artwork. Catalogue seeding owns subsequent presentation.
INSERT INTO badge (id, name, description, sequence) VALUES
    ('mhr', 'MHR', 'Main Hotlap Rank', 10),
    ('nutter', 'Nutter', 'Nutter Rank', 20),
    ('gabor', 'Gabor', 'Completed every chart in Nutter Rank', 30),
    ('world-record-leader', 'World records', 'Leading holders of current world records', 40),
    ('world-record-podium', 'Chart podium', 'Current individual-chart podium positions', 50),
    ('newbie', 'Newbie', 'One published hotlap in this era', 60);
INSERT INTO badge_variant (badge_id, id)
    SELECT b.id, v.id FROM badge b CROSS JOIN (VALUES ('standard'), ('gold'), ('silver'), ('bronze')) v(id)
    WHERE b.id IN ('mhr', 'nutter', 'world-record-leader', 'world-record-podium');
INSERT INTO badge_variant (badge_id, id) VALUES ('gabor', 'standard'), ('newbie', 'standard');
