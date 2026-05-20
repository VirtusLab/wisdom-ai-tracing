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
 *   2. .tracevault/config.toml in the current working directory
 *   3. ~/.config/tracevault/config.toml
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

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ErrorCode,
  McpError,
} from "@modelcontextprotocol/sdk/types.js";
import { readFileSync, existsSync } from "fs";
import { join } from "path";
import { homedir } from "os";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

interface Config {
  serverUrl: string;
  token: string;
  orgSlug: string;
}

function parseToml(content: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#") || trimmed.startsWith("[")) continue;
    const eqIdx = trimmed.indexOf("=");
    if (eqIdx === -1) continue;
    const key = trimmed.slice(0, eqIdx).trim();
    let value = trimmed.slice(eqIdx + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    result[key] = value;
  }
  return result;
}

function loadConfig(): Config {
  // 1. Environment variables take precedence
  const envUrl = process.env.TRACEVAULT_SERVER_URL;
  const envToken = process.env.TRACEVAULT_TOKEN;
  const envOrg = process.env.TRACEVAULT_ORG_SLUG;
  if (envUrl && envOrg) {
    if (!envToken) {
      throw new Error(
        "TraceVault MCP: TRACEVAULT_TOKEN env var is required when using env var config."
      );
    }
    return {
      serverUrl: envUrl.replace(/\/$/, ""),
      token: envToken,
      orgSlug: envOrg,
    };
  }

  // 2. Config file
  const cwd = process.cwd();
  const candidates = [
    join(cwd, ".tracevault", "config.toml"),
    join(homedir(), ".config", "tracevault", "config.toml"),
  ];

  for (const path of candidates) {
    if (!existsSync(path)) continue;
    try {
      const cfg = parseToml(readFileSync(path, "utf8"));
      if (cfg.server_url && cfg.org_slug && cfg.token) {
        return {
          serverUrl: cfg.server_url.replace(/\/$/, ""),
          token: cfg.token,
          orgSlug: cfg.org_slug,
        };
      }
    } catch (err) {
      process.stderr.write(`[tracevault-mcp] Failed to parse config at ${path}: ${err instanceof Error ? err.message : String(err)}\n`);
    }
  }

  throw new Error(
    "TraceVault MCP: no valid configuration found.\n" +
      "Set TRACEVAULT_SERVER_URL, TRACEVAULT_TOKEN (required), TRACEVAULT_ORG_SLUG env vars\n" +
      "or create .tracevault/config.toml with server_url, token (required), and org_slug."
  );
}

// ---------------------------------------------------------------------------
// API client
// ---------------------------------------------------------------------------

interface AskSourceSession {
  session_id: string;
  session_external_id: string;
  repo_name: string;
  user_email: string | null;
  started_at: string | null;
  summary_snippet: string;
}

interface AskResponse {
  answer: string;
  sources: AskSourceSession[];
}

async function askTracevault(
  cfg: Config,
  question: string
): Promise<AskResponse> {
  const url = `${cfg.serverUrl}/api/v1/orgs/${cfg.orgSlug}/chat/ask`;
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  headers["Authorization"] = `Bearer ${cfg.token}`;

  const response = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify({ question }),
  });

  if (!response.ok) {
    const text = await response.text().catch(() => "");
    if (response.status === 403) {
      throw new McpError(
        ErrorCode.InvalidRequest,
        `TraceVault: chat feature not available for this organization (${response.status}). ` +
          "Chat indexing requires an enterprise plan or the chat_search feature flag."
      );
    }
    if (response.status === 401) {
      throw new McpError(
        ErrorCode.InvalidRequest,
        "TraceVault: authentication failed. Check your token in config.toml."
      );
    }
    throw new McpError(
      ErrorCode.InternalError,
      `TraceVault API error ${response.status}: ${text}`
    );
  }

  return response.json() as Promise<AskResponse>;
}

// ---------------------------------------------------------------------------
// Format response
// ---------------------------------------------------------------------------

function formatResponse(resp: AskResponse): string {
  let out = resp.answer;

  if (resp.sources.length > 0) {
    out += "\n\n---\n**Sources**\n";
    for (const s of resp.sources.slice(0, 5)) {
      const when = s.started_at
        ? new Date(s.started_at).toLocaleDateString()
        : "unknown date";
      const who = s.user_email ?? "unknown";
      out += `- Session \`${s.session_external_id}\` (${s.repo_name}, ${who}, ${when})\n`;
      if (s.summary_snippet) {
        const snippet = s.summary_snippet.length > 120
          ? s.summary_snippet.slice(0, 120) + "…"
          : s.summary_snippet;
        out += `  ${snippet}\n`;
      }
    }
  }

  return out;
}

// ---------------------------------------------------------------------------
// MCP server
// ---------------------------------------------------------------------------

const TOOL_NAME = "ask_tracevault";
const TOOL_DESCRIPTION =
  "Query the TraceVault project knowledge base. TraceVault indexes AI coding " +
  "session transcripts, commits, and tool calls so you can ask questions about " +
  "what was built, why decisions were made, who worked on what, and when changes " +
  "happened. Use this when you need project history context that isn't available " +
  "in the current codebase — e.g. 'Why was this module refactored?', " +
  "'What sessions touched the auth service last month?', " +
  "'What decisions were made about the database schema?'.";

async function main(): Promise<void> {
  let config: Config;
  try {
    config = loadConfig();
  } catch (err) {
    process.stderr.write(`${err instanceof Error ? err.message : String(err)}\n`);
    process.exit(1);
  }

  const server = new Server(
    { name: "tracevault", version: "0.1.0" },
    { capabilities: { tools: {} } }
  );

  server.setRequestHandler(ListToolsRequestSchema, async () => ({
    tools: [
      {
        name: TOOL_NAME,
        description: TOOL_DESCRIPTION,
        inputSchema: {
          type: "object" as const,
          properties: {
            question: {
              type: "string",
              description:
                "A natural language question about project history, sessions, " +
                "decisions, or code changes. Be specific for better results.",
            },
          },
          required: ["question"],
        },
      },
    ],
  }));

  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    if (request.params.name !== TOOL_NAME) {
      throw new McpError(
        ErrorCode.MethodNotFound,
        `Unknown tool: ${request.params.name}`
      );
    }

    const args = request.params.arguments as Record<string, unknown>;
    const question = args?.question;
    if (typeof question !== "string" || !question.trim()) {
      throw new McpError(
        ErrorCode.InvalidParams,
        "question must be a non-empty string"
      );
    }

    try {
      const resp = await askTracevault(config, question.trim());
      return {
        content: [
          {
            type: "text",
            text: formatResponse(resp),
          },
        ],
      };
    } catch (err) {
      if (err instanceof McpError) throw err;
      throw new McpError(
        ErrorCode.InternalError,
        `TraceVault query failed: ${err instanceof Error ? err.message : String(err)}`
      );
    }
  });

  const transport = new StdioServerTransport();
  await server.connect(transport);
  process.stderr.write("TraceVault MCP server running (stdio)\n");
}

main().catch((err) => {
  process.stderr.write(`Fatal: ${err instanceof Error ? err.message : String(err)}\n`);
  process.exit(1);
});
