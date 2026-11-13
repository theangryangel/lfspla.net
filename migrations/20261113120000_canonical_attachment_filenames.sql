-- Preserve the upload name for display and store the download name separately.
ALTER TABLE attachment ADD COLUMN filename TEXT;
UPDATE attachment SET filename = original_filename;

UPDATE attachment AS a
SET filename = p.lfs_username || '_' || h.track || '_' || h.vehicle || '_'
    || (h.lap_time_ms / 60000)::text
    || lpad(((h.lap_time_ms / 1000) % 60)::text, 2, '0')
    || lpad((h.lap_time_ms % 1000)::text, 3, '0') || '.' || refs.extension
FROM hotlap AS h
JOIN player AS p ON p.id = h.player_id
CROSS JOIN LATERAL (
    VALUES (h.spr_attachment_id, 'spr'), (h.raf_attachment_id, 'raf')
) AS refs(attachment_id, extension)
WHERE a.id = refs.attachment_id;
