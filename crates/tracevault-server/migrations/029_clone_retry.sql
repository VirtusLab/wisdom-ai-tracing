-- Automatic, capped retry of failed clones so transient failures (e.g. a
-- network blip during a redeploy) self-heal without a manual sync.
--
-- clone_retry_count: number of *automatic* retries attempted for the current
--                    error streak; reset to 0 on a successful clone. Manual
--                    syncs are not capped and do not consume this budget.
-- clone_failed_at:   when the current clone error was recorded, used to space
--                    out the backoff schedule.
ALTER TABLE repos ADD COLUMN IF NOT EXISTS clone_retry_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE repos ADD COLUMN IF NOT EXISTS clone_failed_at TIMESTAMPTZ;
