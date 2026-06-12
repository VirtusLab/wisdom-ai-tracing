-- 036_events_uuid_ordering.sql
-- Order events by a client-minted UUIDv7 (`event_uuid`) instead of the
-- `.event_counter`-derived `event_index`. UUIDv7 is time-ordered and stamped at
-- hook-fire time, so it orders events without a shared, race-prone counter file.
--
-- Transition: `event_index` is retained (now nullable) so older clients that
-- still send it keep working and pre-036 rows keep their ordering. Reads order by
-- `event_uuid` when present and fall back to `event_index` (see the
-- `event_uuid NULLS LAST, event_index NULLS LAST, id` ordering in code).

ALTER TABLE events ADD COLUMN IF NOT EXISTS event_uuid UUID;

-- New clients send no event_index; old clients and pre-036 rows still have one.
ALTER TABLE events ALTER COLUMN event_index DROP NOT NULL;

-- Back the new ordering key.
CREATE INDEX IF NOT EXISTS events_session_event_uuid_idx
    ON events (session_id, event_uuid);
