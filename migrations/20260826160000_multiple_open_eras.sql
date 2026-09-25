-- Version requirements remain mutually exclusive, so multiple eras can accept
-- submissions concurrently without making replay classification ambiguous.
DROP INDEX era_single_open_idx;
