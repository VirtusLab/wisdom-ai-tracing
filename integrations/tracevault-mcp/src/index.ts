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

interface Credentials {
  server_url: string;
  token: string;
  email: string;
}

/** Returns the platform-appropriate config directory, matching the Rust `dirs::config_dir()`. */
function configDir(): string {
  if (process.platform === "darwin") {
    return join(homedir(), "Library", "Application Support");
  }
  if (process.platform === "win32") {
    return process.env.APPDATA ?? join(homedir(), "AppData", "Roaming");
  }
  // Linux / other: use XDG_CONFIG_HOME or ~/.config
  return process.env.XDG_CONFIG_HOME ?? join(homedir(), ".config");
}

function loadCredentials(): Credentials | null {
  const path = join(configDir(), "tracevault", "credentials.json");
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf8")) as Credentials;
  } catch {
    return null;
  }
}

function loadTomlConfig(filePath: string): Record<string, string> | null {
  if (!existsSync(filePath)) return null;
  try {
    return parseToml(readFileSync(filePath, "utf8"));
  } catch (err) {
    // Config file exists but failed to read or parse — log and continue
    process.stderr.write(`[tracevault-mcp] Failed to load config from ${filePath}: ${err instanceof Error ? err.message : String(err)}\n`);
    return null;
  }
}

function loadConfig(): Config {
  // 1. Explicit env vars take full precedence — all three must be present
  const envUrl = process.env.TRACEVAULT_SERVER_URL;
  const envToken = process.env.TRACEVAULT_TOKEN ?? process.env.TRACEVAULT_API_KEY;
  const envOrg = process.env.TRACEVAULT_ORG_SLUG;
  if (envUrl || envOrg) {
    // Partial env var config — require all three to be explicit to avoid
    // silently mixing env + file config in unexpected ways
    if (!envUrl) throw new Error("TraceVault MCP: TRACEVAULT_SERVER_URL is required when using env var config.");
    if (!envToken) throw new Error("TraceVault MCP: TRACEVAULT_TOKEN (or TRACEVAULT_API_KEY) is required when using env var config.");
    if (!envOrg) throw new Error("TraceVault MCP: TRACEVAULT_ORG_SLUG is required when using env var config.");
    return {
      serverUrl: envUrl.replace(/\/$/, ""),
      token: envToken,
      orgSlug: envOrg,
    };
  }

  // 2. credentials.json (written by `tracevault login`) + config.toml for org_slug
  const creds = loadCredentials();
  const cwd = process.cwd();
  const localCfg = loadTomlConfig(join(cwd, ".tracevault", "config.toml"));

  if (creds) {
    // org_slug comes from local config.toml; fall back to env var
    const orgSlug = localCfg?.org_slug ?? envOrg;
    if (orgSlug) {
      return {
        serverUrl: (envUrl ?? creds.server_url).replace(/\/$/, ""),
        token: envToken ?? creds.token,
        orgSlug,
      };
    }
  }

  // 3. config.toml with api_key (non-interactive / CI setup)
  if (localCfg?.server_url && localCfg?.org_slug && (envToken ?? localCfg?.api_key)) {
    return {
      serverUrl: localCfg.server_url.replace(/\/$/, ""),
      token: envToken ?? localCfg.api_key!,
      orgSlug: localCfg.org_slug,
    };
  }

  throw new Error(
    "TraceVault MCP: no valid configuration found.\n\n" +
    "Option A — run `tracevault login` in the repo directory (recommended).\n" +
    "Option B — set env vars: TRACEVAULT_SERVER_URL, TRACEVAULT_TOKEN, TRACEVAULT_ORG_SLUG.\n" +
    "Option C — add api_key and org_slug to .tracevault/config.toml."
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
    "Authorization": `Bearer ${cfg.token}`,
  };

  const abort = new AbortController();
  const timeout = setTimeout(() => abort.abort(), 30_000);

  let response: Response;
  try {
    response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify({ question }),
      signal: abort.signal,
    });
  } catch (err) {
    if ((err as Error).name === "AbortError") {
      throw new McpError(ErrorCode.InternalError, "TraceVault request timed out after 30s");
    }
    throw err;
  } finally {
    clearTimeout(timeout);
  }

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
    const shown = resp.sources.slice(0, 5);
    const extra = resp.sources.length - shown.length;
    const countSuffix = extra > 0 ? ` of ${resp.sources.length}` : "";
    out += `\n\n---\n**Sources** (${shown.length}${countSuffix})\n`;
    for (const s of shown) {
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
