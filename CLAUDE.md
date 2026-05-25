# Before every push run cargo fmt, check and clippy

# Use conventional commit messages: https://www.conventionalcommits.org/en/v1.0.0/#specification

# Visdom Trace — pre-push validation

When your work is complete and you are ready to push:

1. Open a validation window:
   tracevault validation-start

2. Run the required validation tools in order:
   - mcp__cargo__cargo_fmt       (format code)
   - mcp__cargo__cargo_check     (clippy + tests — must succeed)
   - mcp__review__agent_review   (self-review of the diff)

   Only these tools should be called between tracevault validation-start and git push.

3. If a tool fails and you need to fix something:
   - Make the fix
   - Run tracevault validation-start again (invalidates the previous window)
   - Rerun ALL validation tools from step 2

4. Once all tools pass, push normally.
