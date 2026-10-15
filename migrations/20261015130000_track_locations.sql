-- Preserve every existing association before replacing the enum check with a FK.
CREATE TABLE track_location (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    sequence INTEGER NOT NULL DEFAULT 0,
    image_attachment_id BIGINT REFERENCES attachment(id)
);
INSERT INTO track_location (id, name, sequence) VALUES
    ('BL', 'Blackwood', 0), ('SO', 'South City', 1), ('FE', 'Fern Bay', 2),
    ('KY', 'Kyoto Ring', 3), ('AS', 'Aston', 4), ('WE', 'Westhill', 5),
    ('RO', 'Rockingham', 6), ('OTHER', 'Other', 7);
ALTER TABLE track DROP CONSTRAINT track_location_check;
ALTER TABLE track ADD CONSTRAINT track_location_fkey
    FOREIGN KEY (location) REFERENCES track_location(id);
CREATE INDEX track_location_idx ON track(location);
CREATE INDEX track_location_image_attachment_idx ON track_location(image_attachment_id);
