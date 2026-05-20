/**
 * TraceVault GSD2 Tracker Extension
 * Streams GSD2 session events to TraceVault as Claude Code protocol v1 events.
 * Reads credentials from ~/.config/tracevault/credentials.json
 */

import { readFileSync, existsSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const ORG_SLUG = "softwaremill";
const REPO_ID  = "44000761-8d22-4256-bd2c-27a0ba278c6f";

function loadCredentials() {
  const path = join(homedir(), ".config", "tracevault", "credentials.json");
  if (!existsSync(path)) return null;
  try { return JSON.parse(readFileSync(path, "utf8")); }
  catch { return null; }
}

async function post(creds, body) {
  const url = `${creds.server_url.replace(/\/$/, "")}/api/v1/orgs/${ORG_SLUG}/repos/${REPO_ID}/stream`;
  try {
    const resp = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json", "Authorization": `Bearer ${creds.token}` },
      body: JSON.stringify(body),
    });
    if (!resp.ok) {
      const text = await resp.text().catch(() => "");
      process.stderr.write(`[tracevault] stream POST failed ${resp.status}: ${text.slice(0, 200)}\n`);
    }
  } catch (err) {
    process.stderr.write(`[tracevault] stream POST error: ${err instanceof Error ? err.message : String(err)}\n`);
  }
}

const sessions = new Map();
function getState(sessionId) {
  let s = sessions.get(sessionId);
  if (!s) { s = { sessionId, eventIndex: 0 }; sessions.set(sessionId, s); }
  return s;
}
function nextIndex(state) { return state.eventIndex++; }

export default function tracevaultTracker(pi) {
  const creds = loadCredentials();
  if (!creds) {
    process.stderr.write("[tracevault] No credentials at ~/.config/tracevault/credentials.json — tracker disabled\n");
    return;
  }

  pi.on("session_start", async (_event, ctx) => {
    const sessionId = ctx.sessionManager.sessionId ?? crypto.randomUUID();
    const state = getState(sessionId);
    await post(creds, {
      protocol_version: 1, tool: "claude-code", event_type: "session_start",
      session_id: sessionId, timestamp: new Date().toISOString(),
      hook_event_name: "Notification", event_index: nextIndex(state),
      model: ctx.model?.id ?? undefined, cwd: ctx.cwd,
    });
  });

  pi.on("tool_execution_end", async (event, ctx) => {
    const sessionId = ctx.sessionManager.sessionId ?? "unknown";
    const state = getState(sessionId);
    const toolInput = "args" in event ? event.args : undefined;
    const toolResponse = event.result
      ? { output: typeof event.result === "string" ? event.result.slice(0, 2048) : JSON.stringify(event.result).slice(0, 2048) }
      : undefined;
    await post(creds, {
      protocol_version: 1, tool: "claude-code", event_type: "tool_use",
      session_id: sessionId, timestamp: new Date().toISOString(),
      hook_event_name: "PostToolUse", tool_name: event.toolName,
      tool_input: toolInput, tool_response: toolResponse,
      event_index: nextIndex(state), cwd: ctx.cwd,
    });
  });

  pi.on("agent_end", async (event, ctx) => {
    const sessionId = ctx.sessionManager.sessionId ?? "unknown";
    const state = getState(sessionId);
    const lastAssistant = [...(event.messages ?? [])].reverse().find((m) => m.role === "assistant");
    if (!lastAssistant?.usage) return;
    const { usage, model } = lastAssistant;
    // Piggyback token usage as a CC-format transcript line
    const transcriptLine = {
      type: "assistant",
      message: {
        model: model ?? ctx.model?.id ?? "unknown",
        usage: {
          input_tokens: usage.input,
          output_tokens: usage.output,
          cache_read_input_tokens: usage.cacheRead,
          cache_creation_input_tokens: usage.cacheWrite,
        },
      },
      timestamp: new Date().toISOString(),
    };
    await post(creds, {
      protocol_version: 1, tool: "claude-code", event_type: "tool_use",
      session_id: sessionId, timestamp: new Date().toISOString(),
      hook_event_name: "PostToolUse", tool_name: "__token_update__",
      event_index: nextIndex(state), transcript_lines: [transcriptLine], cwd: ctx.cwd,
    });
  });

  pi.on("stop", async (_event, ctx) => {
    const sessionId = ctx.sessionManager.sessionId ?? "unknown";
    const state = getState(sessionId);
    const stats = ctx.sessionManager.getSessionStats?.();
    await post(creds, {
      protocol_version: 1, tool: "claude-code", event_type: "session_end",
      session_id: sessionId, timestamp: new Date().toISOString(),
      hook_event_name: "Stop", event_index: nextIndex(state), cwd: ctx.cwd,
      final_stats: stats ? {
        total_tokens: stats.tokens?.total ?? null,
        input_tokens: stats.tokens?.input ?? null,
        output_tokens: stats.tokens?.output ?? null,
        cache_read_tokens: stats.tokens?.cacheRead ?? null,
        cache_write_tokens: stats.tokens?.cacheWrite ?? null,
        total_tool_calls: stats.toolCalls ?? null,
        user_messages: stats.userMessages ?? null,
        assistant_messages: stats.assistantMessages ?? null,
        duration_ms: null,
      } : null,
    });
    sessions.delete(sessionId);
  });

  process.stderr.write("[tracevault] tracker active — streaming to softwaremill/tracevault\n");
}
