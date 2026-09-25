ALTER TABLE era
    ADD COLUMN track_policy TEXT NOT NULL DEFAULT 'explicit'
        CHECK (track_policy IN ('explicit', 'any')),
    ADD COLUMN vehicle_policy TEXT NOT NULL DEFAULT 'explicit'
        CHECK (vehicle_policy IN ('explicit', 'any'));
