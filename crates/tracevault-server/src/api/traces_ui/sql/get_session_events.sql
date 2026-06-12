SELECT id, event_index, event_type, tool_name, tool_input, tool_response, timestamp
FROM events
WHERE session_id = $1
ORDER BY event_uuid ASC NULLS LAST, event_index ASC NULLS LAST, id ASC
