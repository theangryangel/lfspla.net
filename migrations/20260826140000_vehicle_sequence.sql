-- Standard vehicles follow the canonical LFS declaration order. Mods remain
-- name-sorted within their separate catalogue group.
ALTER TABLE vehicle
    ADD COLUMN sequence INTEGER NOT NULL DEFAULT 0;
