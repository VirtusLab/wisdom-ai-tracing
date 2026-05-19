# TraceVault MCP Server

Exposes the TraceVault chatbot as an MCP tool so any LLM-powered coding harness (Claude Code, Codex CLI, GSD2, Cursor, etc.) can query indexed project session history.

## What it does

TraceVault indexes AI coding session transcripts, commits, and tool calls. This MCP server wraps the chat API so an LLM can ask questions like:

- "Why was `auth.rs` refactored last month?"
- "What sessions touched the payment service recently?"
- "What decisions were made about the database schema?"
- "Who worked on the API layer and what did they change?"

The chatbot retrieves grounded answers from actual session history — not hallucination.

## Installation

```sh
cd integrations/tracevault-mcp
npm install
npm run build
```

## Configuration

The server reads config from (in order):

1. Environment variables: `TRACEVAULT_SERVER_URL`, `TRACEVAULT_TOKEN`, `TRACEVAULT_ORG_SLUG`
2. `.tracevault/config.toml` in the current working directory
3. `~/.config/tracevault/config.toml`

Example `.tracevault/config.toml`:
```toml
server_url = "http://localhost:3000"
token      = "tvs_..."
org_slug   = "my-org"
```

## Add to Claude Code

In `.claude/settings.json`:
```json
{
  "mcpServers": {
    "tracevault": {
      "command": "node",
      "args": ["/absolute/path/to/tracevault-mcp/dist/index.js"]
    }
  }
}
```

## Add to Codex CLI

In `.codex/config.toml`:
```toml
[[mcp_servers]]
name = "tracevault"
command = "node"
args = ["/absolute/path/to/tracevault-mcp/dist/index.js"]
```

## Requirements

- TraceVault server with chat/RAG enabled (enterprise feature)
- Chat indexing must have run (`tracevault chat backfill` or automatic indexing)
- Node.js 18+

## Tool

| Name | Description |
|---|---|
| `ask_tracevault` | Query project session history with a natural language question |
