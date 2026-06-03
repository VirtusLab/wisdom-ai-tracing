# Use conventional commit messages: https://www.conventionalcommits.org/en/v1.0.0/#specification

# Visdom Trace — agent policies

The pre-push and verification-phase rules for this repository are **not** hardcoded
here — they are configured on the TraceVault server and may change. Fetch them at the
start of the session and follow them. They take precedence over any manual instructions.

Call the `mcp__tracevault__agent_policies` MCP tool to fetch those policies.

Treat the printed output as binding rules: it tells you which tools must be called
before push, which must succeed, which file patterns trigger conditional checks, and how
the verification phase gates the push. Re-fetch if you are unsure what currently applies.
