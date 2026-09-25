CREATE TABLE era_badge_refresh (
    era_id BIGINT PRIMARY KEY REFERENCES era (id) ON DELETE CASCADE
);

-- Repair any badges left stale by earlier interrupted or overlapping rebuilds.
INSERT INTO era_badge_refresh (era_id) SELECT id FROM era;
