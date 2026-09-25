-- The replay's raw 16-bit player flags become the source of truth for the
-- control settings, and the six derived booleans that read like independent
-- facts about the lap are computed at read time instead of being stored.
--
-- `steering` stays a column: it is the only control with a SQL filter, and it is
-- a precedence chain (MOUSE > KB_STABILISED > KB_NO_HELP > wheel) rather than a
-- bit test, so deriving it in SQL would put that rule in a second language.
--
-- `abs_enabled` stays a column because it is not a player flag at all: it is its
-- own SPR header byte. It becomes nullable because not every source can report
-- it -- LFSWorld's P-Q and R-X exports have no `abs` column, and LFS reports
-- zero before 0.5Z25 -- and recording that absence as `false` asserted an
-- observation the source never made. `PlayerFlags::SHIFTER` has the same
-- problem, set in 0 of 43,437 0.5P-0.5Q rows while flags as rare as KB_NO_HELP
-- register 115 there; both absences are now resolved against the replay's
-- version when the row is read.

ALTER TABLE hotlap
    -- INTEGER rather than SMALLINT: bit 15 does not fit a signed 16-bit column.
    ADD COLUMN player_flags INTEGER,
    ALTER COLUMN abs_enabled DROP NOT NULL;

-- The columns being dropped encode their bits exactly, so the raw value is
-- recovered rather than invented. Bits 1 and 2 -- set on two thirds of the
-- archived 0.5P-0.5X rows and unnamed in the current protocol model -- were
-- never stored by the old schema, and only a re-run of the import restores them.
UPDATE hotlap
SET player_flags =
      (CASE WHEN driver_side = 'left' THEN 1   ELSE 0 END)
    | (CASE WHEN automatic_gears      THEN 8   ELSE 0 END)
    | (CASE WHEN manual_shifter       THEN 16  ELSE 0 END)
    | (CASE WHEN brake_help_enabled   THEN 64  ELSE 0 END)
    | (CASE WHEN axis_clutch          THEN 128 ELSE 0 END)
    | (CASE WHEN automatic_clutch     THEN 512 ELSE 0 END)
    | (CASE steering
           WHEN 'mouse'               THEN 1024
           WHEN 'keyboard'            THEN 2048
           WHEN 'keyboard_stabilised' THEN 4096
           ELSE 0
       END);

ALTER TABLE hotlap
    ALTER COLUMN player_flags SET NOT NULL,
    DROP COLUMN brake_help_enabled,
    DROP COLUMN automatic_gears,
    DROP COLUMN manual_shifter,
    DROP COLUMN axis_clutch,
    DROP COLUMN automatic_clutch,
    DROP COLUMN driver_side;
