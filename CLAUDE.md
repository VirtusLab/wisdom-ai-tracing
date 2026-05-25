# Before every push run cargo fmt, check and clippy

# Use conventional commit messages: https://www.conventionalcommits.org/en/v1.0.0/#specification

# Visdom Trace — session tracking

At the end of every session (before finishing), run:
    node tools/pi-tracevault-bridge/index.js session-end

This parses the Claude transcript and streams all tool calls and token usage
to the Visdom Trace server, giving full session visibility.

# Visdom Trace — pre-push validation

## During work (session-scoped policies)

These tools must be called at some point during the session before pushing:
- mcp__cargo__cargo_fmt       (format code — call before committing)
- mcp__cargo__cargo_check     (clippy + tests — must succeed)

## When work is complete (validation window)

When ready to push, open a validation window and run the review tool:

1. tracevault validation-start
2. mcp__review__agent_review   (self-review of the diff — validation window policy)

Only allowed tools should be called between tracevault validation-start and git push.
Allowed tools in the validation window:
- mcp__review__agent_review   (required — must be called)
- Read                        (allowed — may be called as needed)

If you need to fix something after the review:
- Make the fix
- Run mcp__cargo__cargo_fmt and mcp__cargo__cargo_check again (session-scoped)
- Run tracevault validation-start again (invalidates previous window)
- Run mcp__review__agent_review again
- Push
