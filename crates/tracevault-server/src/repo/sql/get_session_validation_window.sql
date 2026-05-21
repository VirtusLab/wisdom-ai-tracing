SELECT validation_window_started_at
FROM sessions
WHERE repo_id = $1 AND session_id = $2
ORDER BY started_at DESC
LIMIT 1
