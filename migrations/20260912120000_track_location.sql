ALTER TABLE track
    ADD COLUMN location TEXT NOT NULL DEFAULT 'other' CHECK (
        location IN ('bl', 'so', 'fe', 'ky', 'as', 'we', 'ro', 'other')
    );
