ALTER TABLE ranking ADD COLUMN discipline TEXT NOT NULL DEFAULT 'circuit';

-- Preserve the existing drag ranking until the catalogue is next seeded.
UPDATE ranking SET discipline = 'drag' WHERE slug = 'drag';
