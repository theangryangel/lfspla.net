ALTER TABLE hotlap
    ADD CONSTRAINT hotlap_era_fk
        FOREIGN KEY (era_id) REFERENCES era (id) ON DELETE RESTRICT,
    ADD CONSTRAINT hotlap_track_fk
        FOREIGN KEY (track) REFERENCES track (id) ON DELETE RESTRICT,
    ADD CONSTRAINT hotlap_vehicle_fk
        FOREIGN KEY (vehicle) REFERENCES vehicle (id) ON DELETE RESTRICT,
    ADD CONSTRAINT hotlap_era_track_fk
        FOREIGN KEY (era_id, track)
        REFERENCES era_track (era_id, track_id) ON DELETE RESTRICT,
    ADD CONSTRAINT hotlap_era_vehicle_fk
        FOREIGN KEY (era_id, vehicle)
        REFERENCES era_vehicle (era_id, vehicle_id) ON DELETE RESTRICT;

ALTER TABLE hotlap_submission
    ADD CONSTRAINT hotlap_submission_era_fk
        FOREIGN KEY (era_id) REFERENCES era (id) ON DELETE RESTRICT,
    ADD CONSTRAINT hotlap_submission_track_fk
        FOREIGN KEY (track) REFERENCES track (id) ON DELETE RESTRICT,
    ADD CONSTRAINT hotlap_submission_vehicle_fk
        FOREIGN KEY (vehicle) REFERENCES vehicle (id) ON DELETE RESTRICT,
    ADD CONSTRAINT hotlap_submission_era_track_fk
        FOREIGN KEY (era_id, track)
        REFERENCES era_track (era_id, track_id) ON DELETE RESTRICT;

DROP TABLE vehicle_mod;
