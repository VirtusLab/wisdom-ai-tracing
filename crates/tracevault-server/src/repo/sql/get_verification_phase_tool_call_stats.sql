SELECT
    tool_name,
    tool_input ->> 'command' AS command,
    is_error
FROM events
WHERE session_id = $1
  AND event_type = 'tool_use'
  AND tool_name IS NOT NULL
  AND hook_event_name = 'PostToolUse'
  AND timestamp > $2
