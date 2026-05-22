#!/usr/bin/env node
/**
 * TraceVault MCP Server
 *
 * Exposes the TraceVault chatbot as an MCP tool so any LLM-powered coding
 * harness (Claude Code, Codex CLI, GSD2, Cursor, etc.) can query indexed
 * project session history.
 *
 * Transport: stdio (standard MCP convention for local servers)
 *
 * Configuration (searched in order):
 *   1. Environment variables: TRACEVAULT_SERVER_URL, TRACEVAULT_TOKEN,
 *      TRACEVAULT_ORG_SLUG
 *   2. ~/.config/tracevault/credentials.json (written by `tracevault login`)
 *      combined with .tracevault/config.toml for org_slug
 *   3. TRACEVAULT_API_KEY env var + .tracevault/config.toml (API key auth)
 *
 * Usage:
 *   # Add to .mcp.json or .claude/settings.json:
 *   {
 *     "tracevault": {
 *       "command": "node",
 *       "args": ["/path/to/tracevault-mcp/dist/index.js"]
 *     }
 *   }
 *
 *   # Or after npm install -g @tracevault/mcp-server:
 *   { "tracevault": { "command": "tracevault-mcp" } }
 */
export {};
