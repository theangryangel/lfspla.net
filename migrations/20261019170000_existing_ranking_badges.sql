-- Bootstrap every migrated ranking reference, including operator-authored and
-- demo rankings. These are presentation placeholders, not eligibility rules.
-- Catalogue definitions can subsequently replace names and supply artwork.
WITH referenced AS (
    SELECT badge ->> 'id' AS id, title AS name, description, era_id, position
    FROM ranking WHERE badge IS NOT NULL
    UNION ALL
    SELECT badge -> 'completion' ->> 'id',
        left(title || ' completion', 120), 'Completed every chart in ' || title, era_id, position
    FROM ranking WHERE badge -> 'completion' ->> 'id' IS NOT NULL
)
INSERT INTO badge (id, name, description)
SELECT DISTINCT ON (id) id, left(name, 120), left(description, 1000)
FROM referenced ORDER BY id, era_id, position
ON CONFLICT (id) DO NOTHING;

WITH referenced AS (
    SELECT badge ->> 'id' AS id FROM ranking WHERE badge IS NOT NULL
)
INSERT INTO badge_variant (badge_id, id)
SELECT DISTINCT referenced.id, variant.id FROM referenced
CROSS JOIN (VALUES ('standard'), ('gold'), ('silver'), ('bronze')) variant(id)
ON CONFLICT DO NOTHING;

INSERT INTO badge_variant (badge_id, id)
SELECT DISTINCT badge -> 'completion' ->> 'id', 'standard'
FROM ranking WHERE badge -> 'completion' ->> 'id' IS NOT NULL
ON CONFLICT DO NOTHING;
