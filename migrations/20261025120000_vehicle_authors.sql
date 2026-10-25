-- Vehicle authors are ordinary players, keyed case-insensitively by LFS username.
ALTER TABLE vehicle
    ADD COLUMN author_player_id BIGINT REFERENCES player(id) ON DELETE SET NULL;

CREATE INDEX vehicle_author_player_id_idx ON vehicle(author_player_id);

-- Populate existing authors without changing any existing player profile.
INSERT INTO player (lfs_username, display_name)
SELECT DISTINCT ON (LOWER(btrim(author_username)))
       btrim(author_username), btrim(author_username)
FROM vehicle
WHERE kind = 'mod' AND NULLIF(btrim(author_username), '') IS NOT NULL
ORDER BY LOWER(btrim(author_username)), btrim(author_username)
ON CONFLICT (LOWER(lfs_username)) DO NOTHING;

UPDATE vehicle AS v
SET author_player_id = p.id
FROM player AS p
WHERE v.kind = 'mod' AND LOWER(btrim(v.author_username)) = LOWER(p.lfs_username);
