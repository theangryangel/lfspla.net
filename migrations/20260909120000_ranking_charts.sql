-- A ranking chart is the sole persisted source of a selected pair. Era-wide
-- combinations are derived from these rows when they are needed.
CREATE TABLE ranking_chart (
    era_id TEXT NOT NULL,
    ranking_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    track_id TEXT NOT NULL REFERENCES track (id) ON DELETE RESTRICT,
    vehicle_id TEXT NOT NULL REFERENCES vehicle (id) ON DELETE RESTRICT,
    PRIMARY KEY (era_id, ranking_id, position),
    UNIQUE (era_id, ranking_id, track_id, vehicle_id),
    FOREIGN KEY (era_id, ranking_id) REFERENCES ranking (era_id, id) ON DELETE CASCADE
);

INSERT INTO ranking_chart (era_id, ranking_id, position, track_id, vehicle_id)
SELECT rc.era_id, rc.ranking_id, rc.position, c.track_id, c.vehicle_id
FROM ranking_combination AS rc
JOIN combination AS c ON c.era_id = rc.era_id AND c.id = rc.combination_id;

CREATE INDEX ranking_chart_pair_idx ON ranking_chart (era_id, track_id, vehicle_id);
DROP TABLE ranking_combination;
DROP TABLE combination;
