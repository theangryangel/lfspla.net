ALTER TABLE era
    ADD COLUMN installation_id TEXT;

UPDATE era
SET installation_id = CASE
    WHEN id IN ('2005-06-24', '2006-04-21', '2007-12-21') THEN '0.7'
    WHEN id = '2026-08-26' THEN '0.8'
    -- Existing operator-authored eras retain the old lookup identity until
    -- their definitions explicitly select another installation.
    ELSE id
END;

ALTER TABLE era
    ALTER COLUMN installation_id SET NOT NULL,
    ADD CONSTRAINT era_installation_id_safe CHECK (
        installation_id ~ '^[a-z0-9._-]+$'
        AND installation_id NOT IN ('.', '..')
    );
