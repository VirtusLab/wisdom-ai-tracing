SELECT DISTINCT ON (session_id) session_id, id, validation_window_started_at
FROM sessions
WHERE repo_id = $1 AND session_id = ANY($2)
ORDER BY session_id, started_at DESC
