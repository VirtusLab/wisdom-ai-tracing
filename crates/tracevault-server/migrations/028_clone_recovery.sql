-- Recovery + diagnostics for repo clones.
--
-- clone_error:      last git clone/fetch failure message, surfaced in the UI so
--                   "Clone failed" is actionable instead of opaque.
-- clone_started_at: when the current 'cloning' attempt began, used to detect
--                   orphaned clones (e.g. the server was redeployed mid-clone)
--                   and allow a stale clone to be retried.
ALTER TABLE repos ADD COLUMN IF NOT EXISTS clone_error TEXT;
ALTER TABLE repos ADD COLUMN IF NOT EXISTS clone_started_at TIMESTAMPTZ;
