# TraceVault GSD 2 Extension

Streams [GSD 2](https://github.com/gsd-build/GSD-2) (pi/GSD-2) session events to a TraceVault instance.

## How it works

Unlike Claude Code and Codex CLI (which use shell hooks), GSD 2 integrates via an in-process TypeScript extension. The extension hooks into:

- `session_start` — registers the session with TraceVault
- `tool_execution_end` — streams each tool call (name, args, result, isError)
- `agent_end` — streams token usage and model after each turn
- `stop` — finalises the session with cumulative stats

File changes are extracted server-side from `write` and `edit` tool_execution_end chunks.

## Installation

```sh
# In your project directory (project-local):
mkdir -p .gsd/extensions
cp -r /path/to/tracevault/integrations/gsd2-extension .gsd/extensions/tracevault

# Or user-wide:
cp -r /path/to/tracevault/integrations/gsd2-extension ~/.gsd/extensions/tracevault
```

## Configuration

Create `.tracevault/config.toml` in your project root (or `~/.config/tracevault/config.toml` globally):

```toml
server_url = "http://localhost:3000"   # Your TraceVault instance
token      = "tvs_..."                 # From: tracevault login
org_slug   = "my-org"
repo_id    = "uuid-of-repo"            # From: tracevault init
agent      = "gsd2"                    # Default: gsd2
```

If the config file is absent the extension is a silent no-op — it won't interfere with normal GSD usage.

## What's tracked

| Signal | Source |
|---|---|
| Session start/end | `session_start` / `stop` events |
| Tool calls (name, args, result, isError) | `tool_execution_end` |
| Token usage (input/output/cache) | `agent_end` messages |
| Model name | `agent_end` / `session_start` |
| File changes (write/edit) | Extracted from tool results server-side |

## Limitations

- `is_error` is natively available (GSD2 sets it on all tool results), so `must_succeed` policies work correctly for GSD2 sessions
- Token attribution is per-turn (aggregated from the last `AssistantMessage.usage` in each `agent_end` event)
