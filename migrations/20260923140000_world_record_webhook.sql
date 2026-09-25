ALTER TABLE webhook DROP CONSTRAINT webhook_event_kind_check;
ALTER TABLE webhook ADD CONSTRAINT webhook_event_kind_check
    CHECK (event_kind IN ('hotlap_validated', 'world_record_set'));
ALTER TABLE webhook_notification DROP CONSTRAINT webhook_notification_event_kind_check;
ALTER TABLE webhook_notification ADD CONSTRAINT webhook_notification_event_kind_check
    CHECK (event_kind IN ('hotlap_validated', 'world_record_set'));
