ALTER TABLE track
    DROP CONSTRAINT track_location_check;

UPDATE track
SET location = upper(location);

ALTER TABLE track
    ALTER COLUMN location SET DEFAULT 'OTHER',
    ADD CONSTRAINT track_location_check CHECK (
        location IN ('BL', 'SO', 'FE', 'KY', 'AS', 'WE', 'RO', 'OTHER')
    );
