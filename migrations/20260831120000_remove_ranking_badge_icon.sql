-- Ranking identity already gives the frontend enough information to choose
-- presentation. Remove icon metadata written by the short-lived server-side
-- icon configuration so strict badge decoding can read the catalogue again.
UPDATE ranking
SET badge = badge - 'icon'
WHERE badge IS NOT NULL
  AND badge ? 'icon';
