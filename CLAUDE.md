# Before every push run cargo fmt, check and clippy

# Use conventional commit messages: https://www.conventionalcommits.org/en/v1.0.0/#specification

# Visdom Trace — pre-push validation

## During work (session-scoped policies)

These tools must be called at some point during the session before pushing:
- mcp__cargo__cargo_fmt       (format code — call before committing)
- mcp__cargo__cargo_check     (clippy + tests — must succeed)

## When work is complete (verification phase)

When ready to push, open a verification phase and run the review tool:

1. tracevault verify-start
2. mcp__review__agent_review   (self-review of the diff — verification phase policy)

Only mcp__review__agent_review and Read should be called between tracevault verify-start and git push.

If you need to fix something after the review:
- Make the fix
- Run mcp__cargo__cargo_fmt and mcp__cargo__cargo_check again (session-scoped)
- Run tracevault verify-start again (invalidates the previous phase)
- Run mcp__review__agent_review again
- Push
