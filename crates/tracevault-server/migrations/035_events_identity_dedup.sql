-- 035_events_identity_dedup.sql
-- Dedup tool events by their intrinsic identity (tool_use_id, hook_event_name)
-- instead of the client-assigned event_index.
--
-- Why: Claude Code runs PreToolUse/PostToolUse hooks concurrently (one process
-- per tool) for parallel tool calls. The CLI's .event_counter is a lock-free
-- read-increment-write, so two concurrent `tracevault stream` processes can both
-- emit the same event_index for *different* tools. Under the old
-- `ON CONFLICT (session_id, event_index) DO NOTHING` the second one was silently
-- dropped -- a real event lost. Keying dedup on the event's identity instead
-- lets the two raced events (distinct tool_use_id) both persist, while still
-- collapsing genuine re-deliveries.
--
-- Forward-safe: no existing session has duplicate event_index values (the old
-- unique forbade them), so the `, id` ordering tiebreak added in code does not
-- change the order -- and therefore the seal -- of any already-sealed session.

-- 1. Remove pre-existing identity duplicates that the new unique index would
--    reject. Only rows the index actually enforces: tool_use_id NOT NULL and
--    hook_event_name NOT NULL (NULLs are distinct in a unique index, and legacy
--    NULL-hook Pre/Post pairs must NOT be collapsed). Keep the earliest by
--    event_index, then id. file_changes rows cascade-delete (ON DELETE CASCADE).
DELETE FROM events a
USING events b
WHERE a.tool_use_id IS NOT NULL
  AND a.hook_event_name IS NOT NULL
  AND a.session_id = b.session_id
  AND a.tool_use_id = b.tool_use_id
  AND a.hook_event_name = b.hook_event_name
  AND (a.event_index > b.event_index
       OR (a.event_index = b.event_index AND a.id > b.id));

-- 2. Drop the full unique on (session_id, event_index): it forbids two raced
--    parallel-tool events that share an index but are different tools.
ALTER TABLE events DROP CONSTRAINT IF EXISTS events_session_id_event_index_key;

-- 3. Identity dedup for tool events carrying a tool_use_id.
CREATE UNIQUE INDEX IF NOT EXISTS events_identity_uniq
    ON events (session_id, tool_use_id, hook_event_name)
    WHERE tool_use_id IS NOT NULL;

-- 4. Legacy fallback: keep event_index dedup ONLY for rows without a tool_use_id
--    (pre-tool_use_id data / clients that don't send it).
CREATE UNIQUE INDEX IF NOT EXISTS events_legacy_index_uniq
    ON events (session_id, event_index)
    WHERE tool_use_id IS NULL;

-- 5. event_index is still the ordering key; keep it indexed for ORDER BY now
--    that the dropped unique no longer provides that index.
CREATE INDEX IF NOT EXISTS events_session_event_index_idx
    ON events (session_id, event_index);
