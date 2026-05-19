-- Add is_error column to events table.
-- NULL = unknown (transcript was truncated or not available).
-- false = tool completed successfully.
-- true = tool returned an error (MCP is_error flag or equivalent).
ALTER TABLE events ADD COLUMN IF NOT EXISTS is_error BOOLEAN;
