-- Rename the user-facing "validation window" concept to "verification
-- phase". The old name was unclear in demos — people read "window" as
-- a passive time range and missed that the feature is an active gate
-- that locks the agent into verification mode before push.
--
-- Breaking change: the policies.scope value, the repos column, and the
-- sessions column all rename together so the wire format and the DB
-- stay aligned. No back-compat aliases.

-- 1. sessions: rename the timestamp column.
ALTER TABLE sessions
    RENAME COLUMN validation_window_started_at TO verification_phase_started_at;

-- 2. policies: swap the CHECK constraint BEFORE migrating rows. The old
-- constraint only permits 'validation_window', so updating a row to
-- 'verification_phase' while it is still active raises a check violation.
-- A fresh DB has no such rows (the UPDATE is a no-op), so this only fails on
-- a populated prod DB — drop first, then migrate, then re-add the constraint.
ALTER TABLE policies
    DROP CONSTRAINT IF EXISTS policies_scope_check;
UPDATE policies SET scope = 'verification_phase' WHERE scope = 'validation_window';
ALTER TABLE policies
    ADD CONSTRAINT policies_scope_check
        CHECK (scope IN ('session', 'verification_phase', 'both'));

-- 3. repos: rename the per-repo mode column and reapply the CHECK.
ALTER TABLE repos
    RENAME COLUMN validation_window_mode TO verification_phase_mode;

ALTER TABLE repos
    DROP CONSTRAINT IF EXISTS repos_validation_window_mode_check;
ALTER TABLE repos
    ADD CONSTRAINT repos_verification_phase_mode_check
        CHECK (verification_phase_mode IN ('disabled', 'warn', 'block'));
