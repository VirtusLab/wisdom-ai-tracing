SELECT id, event_index, event_type, tool_name, tool_input, tool_response, timestamp
FROM events
WHERE session_id = $1
ORDER BY event_index ASC, id ASC
