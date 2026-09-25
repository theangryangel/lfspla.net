-- Recorded laps no longer have to name a track or vehicle listed in
-- `era_track`/`era_vehicle`. Those pairing tables are only populated for an
-- `explicit` era policy (plus the rows ranking charts need), so under an `any`
-- policy every lap on an unlisted vehicle -- a mod, typically -- violated these
-- constraints even though the era admits it. Eligibility is decided by the era
-- policy in the application before a submission is promoted.
--
-- `ranking_chart` keeps its pairing foreign keys: charts must name something
-- the era actually admits, which is why era setup still writes chart rows for
-- an `any` policy.
ALTER TABLE hotlap
    DROP CONSTRAINT IF EXISTS hotlap_era_track_fk,
    DROP CONSTRAINT IF EXISTS hotlap_era_vehicle_fk;

ALTER TABLE hotlap_submission
    DROP CONSTRAINT IF EXISTS hotlap_submission_era_track_fk;
