-- Identity path: events with a tool_use_id dedup on their intrinsic identity
-- (session_id, tool_use_id, hook_event_name), matching the partial unique
-- `events_identity_uniq` (WHERE tool_use_id IS NOT NULL). This keys dedup on
-- *what the event is* rather than the client-assigned event_index, so a re-fired
-- or re-delivered hook collapses regardless of its index, while two raced
-- parallel-tool events (distinct tool_use_id) both persist.
INSERT INTO events (session_id, event_index, event_type, tool_name, tool_input, tool_response, is_error, timestamp, hook_event_name, tool_use_id)
VALUES ($1, $2, 'tool_use', $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (session_id, tool_use_id, hook_event_name) WHERE tool_use_id IS NOT NULL DO NOTHING
RETURNING id
