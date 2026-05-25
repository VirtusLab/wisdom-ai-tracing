SELECT COUNT(*) FROM (
    SELECT DISTINCT ON (file_path, change_type, COALESCE(diff_text, ''))
           id
    FROM file_changes
    WHERE session_id = $1
    ORDER BY file_path, change_type, COALESCE(diff_text, ''), timestamp DESC
) sub
