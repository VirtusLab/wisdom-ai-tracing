SELECT session_id, id, validation_window_started_at
FROM sessions
WHERE repo_id = $1 AND session_id = ANY($2)
ORDER BY started_at DESC
