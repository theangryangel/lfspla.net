-- One conversation per lap. A non-null reason makes an entry a report.
CREATE TABLE hotlap_comment (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    original_hotlap_id BIGINT NOT NULL,
    hotlap_id BIGINT REFERENCES hotlap(id) ON DELETE SET NULL,
    parent_id BIGINT REFERENCES hotlap_comment(id) ON DELETE RESTRICT,
    root_id BIGINT REFERENCES hotlap_comment(id) ON DELETE RESTRICT,
    depth SMALLINT NOT NULL DEFAULT 0,
    author_id BIGINT NOT NULL REFERENCES player(id) ON DELETE RESTRICT,
    body TEXT,
    reason VARCHAR(32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    resolved_by BIGINT REFERENCES player(id) ON DELETE RESTRICT
);
CREATE UNIQUE INDEX hotlap_report_player_unique
    ON hotlap_comment (hotlap_id, author_id)
    WHERE reason IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX hotlap_comment_lap ON hotlap_comment (hotlap_id, parent_id, id);
CREATE INDEX hotlap_comment_parent ON hotlap_comment (parent_id, id);
CREATE INDEX hotlap_comment_reports ON hotlap_comment (created_at DESC, id DESC)
    WHERE reason IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX hotlap_comment_root ON hotlap_comment (root_id, id);
