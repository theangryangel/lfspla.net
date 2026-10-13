-- PostgreSQL generates UUIDv4 identifiers for both existing and new attachments.
-- Numeric IDs remain the internal primary keys used by foreign keys.
ALTER TABLE attachment
    ADD COLUMN public_id UUID NOT NULL DEFAULT gen_random_uuid(),
    ADD CONSTRAINT attachment_public_id_unique UNIQUE (public_id);
