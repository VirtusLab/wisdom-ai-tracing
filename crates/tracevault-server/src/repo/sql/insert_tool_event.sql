-- Legacy path: events without a tool_use_id dedup on (session_id, event_index),
-- matching the partial unique `events_legacy_index_uniq` (WHERE tool_use_id IS NULL).
-- Used only when tool_use_id is absent; the identity path is preferred otherwise.
INSERT INTO events (session_id, event_index, event_type, tool_name, tool_input, tool_response, is_error, timestamp, hook_event_name, tool_use_id)
VALUES ($1, $2, 'tool_use', $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (session_id, event_index) WHERE tool_use_id IS NULL DO NOTHING
RETURNING id
