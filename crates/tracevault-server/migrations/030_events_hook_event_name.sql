-- Persist the Claude Code hook phase (PreToolUse / PostToolUse) on tool-use
-- events. Needed so the verification-phase gate can count only COMPLETED tool
-- calls (PostToolUse) and ignore the in-flight terminating `git push` (whose
-- PostToolUse cannot exist while the pre-push check is running). Nullable;
-- rows written before this migration keep NULL and are ignored by the gate.
ALTER TABLE events ADD COLUMN hook_event_name TEXT;
