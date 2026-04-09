-- 017_chat_indexing_chunk_count.sql
-- Track indexed chunk count so backfill can detect sessions with new transcript data.

ALTER TABLE chat_indexing_status
    ADD COLUMN IF NOT EXISTS indexed_chunk_count INTEGER NOT NULL DEFAULT 0;
