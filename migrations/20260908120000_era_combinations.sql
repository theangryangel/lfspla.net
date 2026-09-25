-- Eligibility is the union of ranking combinations, never independent axes.
CREATE TABLE combination (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    era_id TEXT NOT NULL REFERENCES era (id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES track (id) ON DELETE RESTRICT,
    vehicle_id TEXT NOT NULL REFERENCES vehicle (id) ON DELETE RESTRICT,
    UNIQUE (era_id, track_id, vehicle_id),
    UNIQUE (era_id, id)
);

INSERT INTO combination (era_id, track_id, vehicle_id)
SELECT DISTINCT era_id, track_id, vehicle_id FROM ranking_chart
ORDER BY era_id, track_id, vehicle_id;

CREATE TABLE ranking_combination (
    era_id TEXT NOT NULL,
    ranking_id TEXT NOT NULL,
    combination_id BIGINT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (era_id, ranking_id, combination_id),
    UNIQUE (era_id, ranking_id, position),
    FOREIGN KEY (era_id, ranking_id) REFERENCES ranking (era_id, id) ON DELETE CASCADE,
    FOREIGN KEY (era_id, combination_id) REFERENCES combination (era_id, id) ON DELETE CASCADE
);
INSERT INTO ranking_combination (era_id, ranking_id, combination_id, position)
SELECT chart.era_id, chart.ranking_id, combination.id, chart.position
FROM ranking_chart AS chart
JOIN combination USING (era_id, track_id, vehicle_id);

CREATE INDEX ranking_combination_pair_idx ON ranking_combination (era_id, combination_id);
DROP TABLE ranking_chart;
DROP TABLE era_track;
DROP TABLE era_vehicle;
ALTER TABLE era DROP COLUMN track_policy, DROP COLUMN vehicle_policy;
