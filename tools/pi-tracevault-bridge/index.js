#!/usr/bin/env node
/**
 * pi-tracevault-bridge
 *
 * Allows the pi agent harness to stream tool-call events to the TraceVault
 * server and maintain local session state so that `tracevault check` (the
 * pre-push policy hook) sees the tool calls made during the pi session.
 *
 * How it works:
 *   1. session-start: creates a new session ID, writes local session dir with
 *      metadata.json pointing at a synthetic transcript file, streams
 *      session-start to the server.
 *   2. tool-use: appends an assistant/tool_use entry to the transcript (so
 *      check can count tool calls), then streams the post-tool-use event to
 *      the server.
 *   3. session-end: streams the stop event to the server.
 *
 * Usage (from repo root):
 *   node tools/pi-tracevault-bridge/index.js session-start
 *   node tools/pi-tracevault-bridge/index.js tool-use \
 *       --tool-name mcp__cargo__cargo_fmt \
 *       --tool-use-id <uuid> \
 *       --is-error false \
 *       --input '{}' \
 *       --response 'formatted successfully'
 *   node tools/pi-tracevault-bridge/index.js session-end
 *   node tools/pi-tracevault-bridge/index.js session-id
 */

import { readFileSync, writeFileSync, mkdirSync, existsSync, appendFileSync } from "fs";
import { resolve, relative, dirname, isAbsolute } from "path";
import { fileURLToPath } from "url";
import { randomUUID } from "crypto";
import { spawnSync } from "child_process";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "../..");
const PI_SESSION_DIR = resolve(REPO_ROOT, ".tracevault/pi-session");
const SESSION_ID_FILE = resolve(PI_SESSION_DIR, "SESSION_ID");

function ensureDir(dir) {
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
}

function getOrCreateSessionId() {
  ensureDir(PI_SESSION_DIR);
  if (existsSync(SESSION_ID_FILE)) {
    return readFileSync(SESSION_ID_FILE, "utf8").trim();
  }
  const id = randomUUID();
  writeFileSync(SESSION_ID_FILE, id);
  return id;
}

/** Full path to the local session directory inside .tracevault/sessions/<id>/ */
function sessionDir(sessionId) {
  const base = resolve(REPO_ROOT, ".tracevault/sessions");
  const target = resolve(base, sessionId);
  const rel = relative(base, target);
  if (rel.startsWith("..") || isAbsolute(rel)) {
    throw new Error(`Invalid session identifier: ${sessionId}`);
  }
  return target;
}

/** Full path to the synthetic transcript file for this session. */
function transcriptPath(sessionId) {
  return resolve(sessionDir(sessionId), "transcript.jsonl");
}

/**
 * Create the local session directory and metadata.json that `tracevault check`
 * reads to find the transcript path and count tool calls.
 */
function initLocalSession(sessionId) {
  const dir = sessionDir(sessionId);
  ensureDir(dir);
  const tPath = transcriptPath(sessionId);
  // Initialise empty transcript
  if (!existsSync(tPath)) writeFileSync(tPath, "");
  // Write metadata.json so collect_session_data() can find the transcript
  const meta = { transcript_path: tPath };
  writeFileSync(resolve(dir, "metadata.json"), JSON.stringify(meta));
}

/**
 * Append an assistant message with a tool_use block to the transcript.
 * `tracevault check` scans for type=="assistant" messages containing
 * content[].type=="tool_use" to build the tool_calls map.
 */
function appendToolUseToTranscript(sessionId, toolName, toolUseId, toolInput) {
  const entry = {
    type: "assistant",
    message: {
      role: "assistant",
      content: [
        {
          type: "tool_use",
          id: toolUseId,
          name: toolName,
          input: toolInput ?? {},
        },
      ],
    },
  };
  appendFileSync(transcriptPath(sessionId), JSON.stringify(entry) + "\n");
}

/**
 * Append a user message with a tool_result block to the transcript.
 * `tracevault stream` reads these to extract is_error for the current event.
 */
function appendToolResultToTranscript(sessionId, toolUseId, isError, response) {
  const entry = {
    type: "user",
    message: {
      role: "user",
      content: [
        {
          type: "tool_result",
          tool_use_id: toolUseId,
          is_error: isError,
          content: typeof response === "string" ? response : JSON.stringify(response),
        },
      ],
    },
  };
  appendFileSync(transcriptPath(sessionId), JSON.stringify(entry) + "\n");
}

/**
 * Build the HookEvent JSON that `tracevault stream` reads from stdin.
 */
function buildHookEvent(sessionId, hookEventName, opts = {}) {
  return {
    session_id: sessionId,
    transcript_path: transcriptPath(sessionId),
    cwd: REPO_ROOT,
    permission_mode: "default",
    hook_event_name: hookEventName,
    tool_name: opts.toolName ?? null,
    tool_input: opts.toolInput ?? null,
    tool_response: opts.toolResponse ?? null,
    tool_use_id: opts.toolUseId ?? null,
  };
}

/**
 * Call `tracevault stream --event <type>` with hookEvent as stdin.
 */
function callTraceVaultStream(eventType, hookEvent) {
  const result = spawnSync(
    "tracevault",
    ["stream", "--event", eventType],
    {
      input: JSON.stringify(hookEvent),
      cwd: REPO_ROOT,
      encoding: "utf8",
    }
  );
  return {
    success: result.status === 0,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    status: result.status,
  };
}

// ── CLI ───────────────────────────────────────────────────────────────────────

const [,, command, ...args] = process.argv;

function parseArgs(args) {
  const opts = {};
  for (let i = 0; i < args.length; i++) {
    if (args[i].startsWith("--")) {
      const key = args[i].slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      opts[key] = args[i + 1];
      i++;
    }
  }
  return opts;
}

switch (command) {
  case "session-start": {
    ensureDir(PI_SESSION_DIR);
    const sessionId = randomUUID();
    writeFileSync(SESSION_ID_FILE, sessionId);
    initLocalSession(sessionId);

    const hookEvent = buildHookEvent(sessionId, "Notification");
    const res = callTraceVaultStream("notification", hookEvent);
    if (res.success) {
      console.log(`✅ Session started: ${sessionId}`);
    } else {
      console.error(`⚠️  tracevault stream warning: ${res.stderr.trim()}`);
      console.log(`Session ID: ${sessionId}`);
    }
    break;
  }

  case "session-end": {
    const sessionId = getOrCreateSessionId();
    const hookEvent = buildHookEvent(sessionId, "Stop");
    const res = callTraceVaultStream("stop", hookEvent);
    if (res.success) {
      console.log(`✅ Session ended: ${sessionId}`);
    } else {
      console.error(`⚠️  tracevault stream failed: ${res.stderr.trim()}`);
    }
    break;
  }

  case "tool-use": {
    const opts = parseArgs(args);
    const sessionId = getOrCreateSessionId();
    // Ensure local session state exists (in case session-start was skipped)
    initLocalSession(sessionId);

    const toolUseId = opts.toolUseId ?? randomUUID();
    const isError = opts.isError === "true";
    const toolInput = opts.input ? JSON.parse(opts.input) : null;
    const toolResponse = opts.response ?? "";

    // 1. Write tool_use to transcript (what `check` reads to count tool calls)
    appendToolUseToTranscript(sessionId, opts.toolName, toolUseId, toolInput);

    // 2. Write tool_result to transcript (what `stream` reads to extract is_error)
    appendToolResultToTranscript(sessionId, toolUseId, isError, toolResponse);

    // 3. Stream the event to the server
    const hookEvent = buildHookEvent(sessionId, "PostToolUse", {
      toolName: opts.toolName,
      toolInput,
      toolResponse: { type: "text", text: toolResponse },
      toolUseId,
    });

    const res = callTraceVaultStream("post-tool-use", hookEvent);
    if (res.success) {
      console.log(`✅ Tool event streamed: ${opts.toolName} (${isError ? "error" : "ok"})`);
    } else {
      console.error(`⚠️  tracevault stream failed: ${res.stderr.trim()}`);
    }
    break;
  }

  case "session-id": {
    const sessionId = getOrCreateSessionId();
    console.log(sessionId);
    break;
  }

  default:
    console.error(`Unknown command: ${command}`);
    console.error("Usage: session-start | session-end | tool-use | session-id");
    process.exit(1);
}
