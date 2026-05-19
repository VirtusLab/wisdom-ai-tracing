/**
 * TraceVault GSD 2 Extension
 *
 * Streams session events to a TraceVault server instance.
 * Install by copying or symlinking this extension directory into your
 * GSD extensions path (~/.gsd/extensions/tracevault/ or project-local
 * .gsd/extensions/tracevault/).
 *
 * Configuration (via tracevault.toml in project root or ~/.config/tracevault/config.toml):
 *   server_url = "http://localhost:3000"  # or your TraceVault instance
 *   token      = "tvs_..."               # API token from tracevault login
 *   org_slug   = "my-org"
 *   repo_id    = "uuid-of-repo"
 *   agent      = "gsd2"                  # defaults to "gsd2"
 */

import type { ExtensionAPI } from "@gsd/pi-coding-agent";
import { readFileSync, existsSync } from "fs";
import { join } from "path";
import { homedir } from "os";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

interface TracevaultConfig {
  server_url: string;
  token?: string;
  org_slug: string;
  repo_id: string;
  agent: string;
}

function loadConfig(projectRoot: string): TracevaultConfig | null {
  // Search order: project root → ~/.config/tracevault/
  const candidates = [
    join(projectRoot, ".tracevault", "config.toml"),
    join(homedir(), ".config", "tracevault", "config.toml"),
  ];

  for (const path of candidates) {
    if (!existsSync(path)) continue;
    try {
      const raw = readFileSync(path, "utf8");
      const cfg = parseToml(raw);
      if (cfg.server_url && cfg.org_slug && cfg.repo_id) {
        return {
          server_url: String(cfg.server_url).replace(/\/$/, ""),
          token: cfg.token ? String(cfg.token) : undefined,
          org_slug: String(cfg.org_slug),
          repo_id: String(cfg.repo_id),
          agent: cfg.agent ? String(cfg.agent) : "gsd2",
        };
      }
    } catch {
      // ignore malformed config
    }
  }
  return null;
}

/** Minimal TOML parser — handles flat key = "value" pairs only. */
function parseToml(content: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#") || trimmed.startsWith("[")) continue;
    const eqIdx = trimmed.indexOf("=");
    if (eqIdx === -1) continue;
    const key = trimmed.slice(0, eqIdx).trim();
    let value = trimmed.slice(eqIdx + 1).trim();
    // Strip surrounding quotes
    if ((value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1);
    }
    result[key] = value;
  }
  return result;
}

// ---------------------------------------------------------------------------
// HTTP client
// ---------------------------------------------------------------------------

async function post(
  cfg: TracevaultConfig,
  path: string,
  body: unknown,
): Promise<void> {
  const url = `${cfg.server_url}${path}`;
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  if (cfg.token) {
    headers["Authorization"] = `Bearer ${cfg.token}`;
  }
  const response = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    // Non-fatal — tracing failures must not interrupt the agent
    const text = await response.text().catch(() => "");
    console.error(`[tracevault] POST ${path} failed: ${response.status} ${text}`);
  }
}

// ---------------------------------------------------------------------------
// Session state
// ---------------------------------------------------------------------------

interface SessionState {
  sessionId: string;
  eventIndex: number;
  serverSessionId: string | null;
}

// One state per GSD session (keyed by GSD session ID)
const sessions = new Map<string, SessionState>();

function getState(gsdSessionId: string): SessionState {
  let s = sessions.get(gsdSessionId);
  if (!s) {
    s = { sessionId: gsdSessionId, eventIndex: 0, serverSessionId: null };
    sessions.set(gsdSessionId, s);
  }
  return s;
}

// ---------------------------------------------------------------------------
// Stream helpers
// ---------------------------------------------------------------------------

function streamUrl(cfg: TracevaultConfig): string {
  return `/api/v1/orgs/${cfg.org_slug}/repos/${cfg.repo_id}/stream`;
}

async function sendEvent(
  cfg: TracevaultConfig,
  state: SessionState,
  eventType: string,
  hookEventName: string,
  toolName?: string,
  toolInput?: unknown,
  toolResponse?: unknown,
  toolIsError?: boolean,
  transcriptLines?: unknown[],
  model?: string,
): Promise<void> {
  const payload: Record<string, unknown> = {
    protocol_version: 2,
    tool: cfg.agent,
    event_type: eventType,
    session_id: state.sessionId,
    timestamp: new Date().toISOString(),
    hook_event_name: hookEventName,
    event_index: state.eventIndex++,
  };

  if (toolName !== undefined) payload.tool_name = toolName;
  if (toolInput !== undefined) payload.tool_input = toolInput;
  if (toolResponse !== undefined) payload.tool_response = toolResponse;
  if (toolIsError !== undefined) payload.tool_is_error = toolIsError;
  if (transcriptLines && transcriptLines.length > 0) {
    payload.transcript_lines = transcriptLines;
  }
  if (model) payload.model = model;

  await post(cfg, streamUrl(cfg), payload);
}

// ---------------------------------------------------------------------------
// Extension entry point
// ---------------------------------------------------------------------------

export default function tracevaultGsd2(pi: ExtensionAPI): void {
  let cfg: TracevaultConfig | null = null;

  // ── session_start ─────────────────────────────────────────────────────────
  pi.on("session_start", async (_event, ctx) => {
    cfg = loadConfig(ctx.cwd);
    if (!cfg) return; // Not a TraceVault project — silent no-op

    const state = getState(ctx.sessionManager.sessionId ?? crypto.randomUUID());
    const model = ctx.model?.id ?? undefined;

    await sendEvent(
      cfg,
      state,
      "session_start",
      "session_start",
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      model,
    );
  });

  // ── tool_execution_end ────────────────────────────────────────────────────
  // Fires after every tool call with result + isError.
  pi.on("tool_execution_end", async (event, ctx) => {
    if (!cfg) return;
    const sessionId = ctx.sessionManager.sessionId ?? "unknown";
    const state = getState(sessionId);

    const toolInput = "args" in event ? (event.args as Record<string, unknown>) : undefined;
    const toolResponse = event.result ? { output: JSON.stringify(event.result) } : undefined;

    // Build a transcript chunk for this tool execution so the server can
    // extract file changes (write/edit) and record the event in the DB.
    const chunk = {
      type: "tool_execution_end",
      toolCallId: event.toolCallId,
      toolName: event.toolName,
      args: toolInput,
      result: event.result,
      isError: event.isError,
      timestamp: new Date().toISOString(),
    };

    await sendEvent(
      cfg,
      state,
      "tool_use",
      "tool_execution_end",
      event.toolName,
      toolInput,
      toolResponse,
      event.isError,
      [chunk],
    );
  });

  // ── agent_end ─────────────────────────────────────────────────────────────
  // Fires at the end of each agent loop turn — carries messages with usage.
  pi.on("agent_end", async (event, ctx) => {
    if (!cfg) return;
    const sessionId = ctx.sessionManager.sessionId ?? "unknown";
    const state = getState(sessionId);

    // Find the last assistant message for token usage and model
    const lastAssistant = [...(event.messages ?? [])]
      .reverse()
      .find((m) => m.role === "assistant");

    if (!lastAssistant) return;

    const usage = (lastAssistant as { usage?: { input: number; output: number; cacheRead: number; cacheWrite: number } }).usage;
    const model = (lastAssistant as { model?: string }).model;

    // Build a transcript chunk that the server's extract_token_usage can parse
    const chunk = {
      type: "agent_end",
      usage: usage
        ? {
            input: usage.input,
            output: usage.output,
            cacheRead: usage.cacheRead,
            cacheWrite: usage.cacheWrite,
          }
        : undefined,
      model,
      timestamp: new Date().toISOString(),
    };

    await sendEvent(
      cfg,
      state,
      "tool_use",
      "agent_end",
      undefined,
      undefined,
      undefined,
      undefined,
      [chunk],
      model,
    );
  });

  // ── stop ──────────────────────────────────────────────────────────────────
  // Fires when the agent is truly idle — use as session-end signal.
  pi.on("stop", async (event, ctx) => {
    if (!cfg) return;
    const sessionId = ctx.sessionManager.sessionId ?? "unknown";
    const state = getState(sessionId);

    // Gather final session stats
    const stats = ctx.sessionManager.getSessionStats?.();
    const finalStats = stats
      ? {
          total_tokens: stats.tokens?.total ?? null,
          input_tokens: stats.tokens?.input ?? null,
          output_tokens: stats.tokens?.output ?? null,
          cache_read_tokens: stats.tokens?.cacheRead ?? null,
          cache_write_tokens: stats.tokens?.cacheWrite ?? null,
          total_tool_calls: stats.toolCalls ?? null,
          user_messages: stats.userMessages ?? null,
          assistant_messages: stats.assistantMessages ?? null,
          duration_ms: null, // Not directly available here
        }
      : null;

    const payload: Record<string, unknown> = {
      protocol_version: 2,
      tool: cfg.agent,
      event_type: "session_end",
      session_id: state.sessionId,
      timestamp: new Date().toISOString(),
      hook_event_name: "stop",
      event_index: state.eventIndex++,
    };
    if (finalStats) payload.final_stats = finalStats;

    await post(cfg, streamUrl(cfg), payload);

    // Clean up state
    sessions.delete(sessionId);
  });
}
