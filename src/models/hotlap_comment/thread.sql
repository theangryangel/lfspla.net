-- The caller supplies bound placeholders for the selected comment IDs.
WITH RECURSIVE thread AS (
    SELECT id, original_hotlap_id
    FROM hotlap_comment
    WHERE id IN ({selected_ids})

    UNION

    SELECT child.id, child.original_hotlap_id
    FROM hotlap_comment AS child
    JOIN thread AS parent
      ON child.parent_id = parent.id
     AND child.original_hotlap_id = parent.original_hotlap_id
), descendants AS (
    -- Keep the existing chronological prefix and response size limit.
    -- Sorting may visit the full selected subtree; this bounds returned rows.
    SELECT id
    FROM thread
    WHERE id NOT IN ({selected_ids})
    ORDER BY id
    LIMIT 1000
), included AS (
    SELECT id FROM hotlap_comment WHERE id IN ({selected_ids})
    UNION ALL
    SELECT id FROM descendants
)
SELECT comment.*,
       author.lfs_username,
       author.display_name,
       author.country_code,
       author.flag_code,
       (SELECT COUNT(*) FROM hotlap_comment AS reply
        WHERE reply.parent_id = comment.id) AS reply_count
FROM included
JOIN hotlap_comment AS comment ON comment.id = included.id
JOIN player AS author ON author.id = comment.author_id
ORDER BY comment.id
