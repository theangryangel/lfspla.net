ALTER TABLE player ADD COLUMN profile_links jsonb NOT NULL DEFAULT '{}'::jsonb;
