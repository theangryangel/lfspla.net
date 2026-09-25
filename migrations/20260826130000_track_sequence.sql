-- Track order follows the canonical declaration order from insim_core rather
-- than relying on PostgreSQL's unspecified physical/insertion order.
ALTER TABLE track
    ADD COLUMN sequence INTEGER NOT NULL DEFAULT 0;
