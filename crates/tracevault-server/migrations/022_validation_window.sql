-- Validation window support.
--
-- 1. sessions: track when the agent last declared "I am now in validation mode".
--    Only the most recent timestamp per session matters — re-opening overwrites.
-- 2. policies: add scope ('session' | 'validation_window' | 'both') and
--    extend action to include 'allow' (permitted in window, no count required).
-- 3. repos: add validation_window_mode ('disabled' | 'warn' | 'block') which
--    controls what happens when an unknown tool is called inside the window.

ALTER TABLE sessions
    ADD COLUMN IF NOT EXISTS validation_window_started_at TIMESTAMPTZ;

ALTER TABLE policies
    ADD COLUMN IF NOT EXISTS scope TEXT NOT NULL DEFAULT 'session'
        CHECK (scope IN ('session', 'validation_window', 'both'));

ALTER TABLE repos
    ADD COLUMN IF NOT EXISTS validation_window_mode TEXT NOT NULL DEFAULT 'disabled'
        CHECK (validation_window_mode IN ('disabled', 'warn', 'block'));
