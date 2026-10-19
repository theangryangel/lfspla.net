-- Retire the bookmarks template and use one shared card layout.
DELETE FROM community_card WHERE content->>'type' = 'links';
ALTER TABLE community_card DROP COLUMN layout;
ALTER TABLE community_card DROP CONSTRAINT community_card_content_check;
ALTER TABLE community_card ADD CONSTRAINT community_card_content_check CHECK (
    jsonb_typeof(content) = 'object'
    AND content ? 'type'
    AND content->>'type' IN ('partner', 'image', 'announcement')
);
