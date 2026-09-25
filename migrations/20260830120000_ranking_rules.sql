-- The catalogue currently has one aggregate ranking system. Persist its
-- configurable values directly instead of retaining an opaque discriminator.
ALTER TABLE ranking
    ADD COLUMN benchmark_percent INTEGER,
    ADD COLUMN nation_max_points INTEGER,
    ADD COLUMN nation_driver_limit INTEGER;

UPDATE ranking
SET benchmark_percent = 103,
    nation_max_points = 10,
    nation_driver_limit = 3
WHERE model = 'lfs_world_v1';

ALTER TABLE ranking
    ALTER COLUMN benchmark_percent SET NOT NULL,
    ALTER COLUMN nation_max_points SET NOT NULL,
    ALTER COLUMN nation_driver_limit SET NOT NULL,
    ADD CONSTRAINT ranking_benchmark_percent_check
        CHECK (benchmark_percent > 0),
    ADD CONSTRAINT ranking_nation_max_points_check
        CHECK (nation_max_points > 0),
    ADD CONSTRAINT ranking_nation_driver_limit_check
        CHECK (
            nation_driver_limit > 0
            AND nation_driver_limit <= nation_max_points
        );

ALTER TABLE ranking DROP COLUMN model;
