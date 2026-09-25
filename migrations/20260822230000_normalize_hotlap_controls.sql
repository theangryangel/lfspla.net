ALTER TABLE hotlap
    ADD COLUMN brake_help_enabled BOOLEAN,
    ADD COLUMN automatic_gears BOOLEAN,
    ADD COLUMN manual_shifter BOOLEAN,
    ADD COLUMN axis_clutch BOOLEAN,
    ADD COLUMN automatic_clutch BOOLEAN,
    ADD COLUMN driver_side TEXT;

UPDATE hotlap
SET brake_help_enabled = (flags & 64) <> 0,
    automatic_gears = (flags & 8) <> 0,
    manual_shifter = (flags & 16) <> 0,
    axis_clutch = (flags & 128) <> 0,
    automatic_clutch = (flags & 512) <> 0,
    driver_side = CASE WHEN (flags & 1) <> 0 THEN 'left' ELSE 'right' END;

ALTER TABLE hotlap
    ALTER COLUMN brake_help_enabled SET NOT NULL,
    ALTER COLUMN automatic_gears SET NOT NULL,
    ALTER COLUMN manual_shifter SET NOT NULL,
    ALTER COLUMN axis_clutch SET NOT NULL,
    ALTER COLUMN automatic_clutch SET NOT NULL,
    ALTER COLUMN driver_side SET NOT NULL,
    ADD CONSTRAINT hotlap_driver_side_check CHECK (
        driver_side IN ('left', 'right')
    ),
    DROP COLUMN flags;
