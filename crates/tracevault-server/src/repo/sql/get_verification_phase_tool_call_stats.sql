SELECT
    tool_name,
    COUNT(*)::bigint                                  AS total,
    COUNT(*) FILTER (WHERE is_error = false)::bigint  AS successful
FROM events
WHERE session_id = $1
  AND event_type = 'tool_use'
  AND tool_name IS NOT NULL
  AND timestamp > $2
GROUP BY tool_name
