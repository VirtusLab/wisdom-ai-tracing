SELECT DISTINCT ON (file_path, change_type, COALESCE(diff_text, ''))
       id, file_path, change_type, diff_text, content_hash, timestamp
FROM file_changes
WHERE session_id = $1
ORDER BY file_path, change_type, COALESCE(diff_text, ''), timestamp DESC
