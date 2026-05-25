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
 *   3. session-end: finds the most recent Claude transcript JSONL for this
 *      project, parses it, and streams all tool calls + token usage to the
 *      server before sending the stop event.
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

import {
  readFileSync,
  writeFileSync,
  mkdirSync,
  existsSync,
  appendFileSync,
  readdirSync,
  statSync,
} from "fs";
import { resolve, relative, dirname, isAbsolute, join } from "path";
import { fileURLToPath } from "url";
import { randomUUID } from "crypto";
import { spawnSync } from "child_process";
import { homedir } from "os";

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
  if (!existsSync(tPath)) writeFileSync(tPath, "");
  const meta = { transcript_path: tPath };
  writeFileSync(resolve(dir, "metadata.json"), JSON.stringify(meta));
}

function appendToolUseToTranscript(sessionId, toolName, toolUseId, toolInput) {
  const entry = {
    type: "assistant",
    message: {
      role: "assistant",
      content: [{ type: "tool_use", id: toolUseId, name: toolName, input: toolInput ?? {} }],
    },
  };
  appendFileSync(transcriptPath(sessionId), JSON.stringify(entry) + "\n");
}

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

function callTraceVaultStream(eventType, hookEvent) {
  const result = spawnSync("tracevault", ["stream", "--event", eventType], {
    input: JSON.stringify(hookEvent),
    cwd: REPO_ROOT,
    encoding: "utf8",
  });
  return {
    success: result.status === 0,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    status: result.status,
  };
}

// ── Transcript parsing ────────────────────────────────────────────────────────

/**
 * Derive the Claude project dir for REPO_ROOT.
 * Claude uses the absolute path with '/' replaced by '-'.
 */
function claudeProjectDir() {
  // Claude replaces both '/' and '.' with '-' to form the project directory name
  const projectHash = REPO_ROOT.replace(/[/.]/g, "-");
  return join(homedir(), ".claude", "projects", projectHash);
}

/**
 * Find the most recently modified .jsonl transcript file in the Claude project dir.
 * Returns null if none found.
 */
function findLatestTranscript() {
  const projectDir = claudeProjectDir();
  if (!existsSync(projectDir)) return null;

  const files = readdirSync(projectDir)
    .filter((f) => f.endsWith(".jsonl"))
    .map((f) => {
      const fullPath = join(projectDir, f);
      // Guard: ensure the resolved path stays within the project directory
      const rel = relative(projectDir, fullPath);
      if (rel.startsWith("..") || isAbsolute(rel)) return null;
      return { path: fullPath, mtime: statSync(fullPath).mtimeMs };
    })
    .filter(Boolean)
    .sort((a, b) => b.mtime - a.mtime);

  return files.length > 0 ? files[0].path : null;
}

/**
 * Parse a Claude transcript JSONL file and extract:
 * - toolEvents: array of { toolUseId, toolName, toolInput, toolResponse, isError, timestamp }
 * - tokenTotals: { inputTokens, outputTokens, cacheReadTokens, cacheWriteTokens }
 */
/**
 * Parse transcript JSONL starting at byteOffset (default 0).
 * Returns toolEvents, tokenTotals, and bytesRead (new offset for next call).
 */
function parseTranscript(transcriptFile, byteOffset = 0) {
  // Guard: transcript must live under the Claude projects directory
  const claudeBase = join(homedir(), ".claude", "projects");
  const rel = relative(claudeBase, transcriptFile);
  if (rel.startsWith("..") || isAbsolute(rel)) {
    throw new Error(`Transcript path rejected (outside Claude projects dir): ${transcriptFile}`);
  }
  const fullContent = readFileSync(transcriptFile, "utf8");
  const newContent = fullContent.slice(byteOffset);
  const bytesRead = byteOffset + Buffer.byteLength(newContent, "utf8");
  const lines = newContent.split("\n").filter(Boolean);

  // Map tool_use_id -> tool_use block (from assistant messages)
  const toolUseMap = new Map();
  // Map tool_use_id -> { is_error, content } (from user tool_result messages)
  const toolResultMap = new Map();
  // Timestamps from assistant messages (keyed by index in order)
  const assistantTimestamps = [];

  const tokenTotals = {
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
  };

  for (const line of lines) {
    let obj;
    try {
      obj = JSON.parse(line);
    } catch (err) {
      process.stderr.write(`[tv] skipping malformed transcript line: ${err.message}\n`);
      continue;
    }

    const msgType = obj.type;
    if (!msgType) continue; // guard: skip lines without a type

    if (msgType === "assistant") {
      const msg = obj.message ?? {};
      const ts = obj.timestamp ?? null;

      // Accumulate token usage
      const usage = msg.usage ?? {};
      tokenTotals.inputTokens += usage.input_tokens ?? 0;
      tokenTotals.outputTokens += usage.output_tokens ?? 0;
      tokenTotals.cacheReadTokens += usage.cache_read_input_tokens ?? 0;
      tokenTotals.cacheWriteTokens += usage.cache_creation_input_tokens ?? 0;

      // Collect tool_use blocks
      const content = msg.content ?? [];
      for (const block of content) {
        if (!block || block.type !== "tool_use" || !block.id) continue;
        toolUseMap.set(block.id, { ...block, timestamp: ts });
      }
    }

    if (msgType === "user") {
      const content = obj.message?.content ?? [];
      for (const block of content) {
        if (!block || block.type !== "tool_result" || !block.tool_use_id) continue;
        toolResultMap.set(block.tool_use_id, {
          isError: block.is_error ?? false,
          content: block.content ?? "",
        });
      }
    }
  }

  // Build ordered tool events (in tool_use insertion order)
  const toolEvents = [];
  for (const [toolUseId, toolUse] of toolUseMap) {
    const result = toolResultMap.get(toolUseId) ?? { isError: false, content: "" };
    toolEvents.push({
      toolUseId,
      toolName: toolUse.name,
      toolInput: toolUse.input ?? null,
      toolResponse: result.content,
      isError: result.isError,
      timestamp: toolUse.timestamp,
    });
  }

  return { toolEvents, tokenTotals, bytesRead };
}

/**
 * Stream all tool events from the transcript to the TraceVault server and
 * also append them to the local synthetic transcript (so policy check sees them).
 * realTranscriptPath: the actual Claude transcript JSONL, used so the server
 * can extract token usage from transcript_lines.
 */
function streamTranscriptEvents(sessionId, toolEvents, realTranscriptPath) {
  let streamed = 0;
  let failed = 0;

  for (const ev of toolEvents) {
    // Append to local synthetic transcript for policy check
    appendToolUseToTranscript(sessionId, ev.toolName, ev.toolUseId, ev.toolInput);
    appendToolResultToTranscript(sessionId, ev.toolUseId, ev.isError, ev.toolResponse ?? "");

    // Stream to server — use real transcript path so server can extract tokens
    const hookEvent = {
      session_id: sessionId,
      transcript_path: realTranscriptPath ?? transcriptPath(sessionId),
      cwd: REPO_ROOT,
      permission_mode: "default",
      hook_event_name: "PostToolUse",
      tool_name: ev.toolName,
      tool_input: ev.toolInput,
      tool_response: ev.toolResponse
        ? { type: "text", text: String(ev.toolResponse).slice(0, 4096) }
        : null,
      tool_use_id: ev.toolUseId,
    };

    const res = callTraceVaultStream("post-tool-use", hookEvent);
    if (res.success) {
      streamed++;
    } else {
      failed++;
    }
  }

  return { streamed, failed };
}

// ── CLI ───────────────────────────────────────────────────────────────────────

const [, , command, ...args] = process.argv;

function parseArgs(argList) {
  const opts = {};
  for (let i = 0; i < argList.length; i++) {
    if (argList[i].startsWith("--")) {
      const key = argList[i].slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      opts[key] = argList[i + 1];
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
    initLocalSession(sessionId);

    // Step 1: Incrementally parse the Claude transcript.
    // We track a byte offset so each Stop-hook invocation only processes
    // new lines added since the last run — O(new lines) not O(full transcript).
    const transcriptFile = findLatestTranscript();
    let transcriptForStop = transcriptPath(sessionId); // fallback

    if (transcriptFile) {
      transcriptForStop = transcriptFile;

      // Global offset file keyed by transcript path — shared across all bridge
      // sessions so we never re-stream transcript lines already sent.
      // Format: JSON object { "<transcriptPath>": bytesRead, ... }
      ensureDir(PI_SESSION_DIR);
      const globalOffsetFile = resolve(PI_SESSION_DIR, ".global_transcript_offsets.json");
      let offsets = {};
      if (existsSync(globalOffsetFile)) {
        try {
          offsets = JSON.parse(readFileSync(globalOffsetFile, "utf8"));
        } catch {
          offsets = {};
        }
      }
      const readOffset = offsets[transcriptFile] ?? 0;

      try {
        const { toolEvents, bytesRead } = parseTranscript(transcriptFile, readOffset);

        if (toolEvents.length > 0) {
          process.stderr.write(
            `[tv] +${toolEvents.length} events from transcript (offset ${readOffset}→${bytesRead})\n`
          );
          const { streamed, failed } = streamTranscriptEvents(
            sessionId,
            toolEvents,
            transcriptFile
          );
          if (failed > 0) {
            process.stderr.write(`[tv] ⚠️  ${failed} events failed to stream\n`);
          }
        }

        // Persist updated global offset
        if (bytesRead > readOffset) {
          offsets[transcriptFile] = bytesRead;
          writeFileSync(globalOffsetFile, JSON.stringify(offsets, null, 2));
        }
      } catch (err) {
        process.stderr.write(`[tv] ⚠️  transcript parse failed: ${err.message}\n`);
      }
    }

    // Step 2: Send stop event pointing at the real transcript so the server
    // can extract final token stats from transcript_lines.
    const stopHookEvent = {
      session_id: sessionId,
      transcript_path: transcriptForStop,
      cwd: REPO_ROOT,
      permission_mode: "default",
      hook_event_name: "Stop",
      tool_name: null,
      tool_input: null,
      tool_response: null,
      tool_use_id: null,
    };
    const res = callTraceVaultStream("stop", stopHookEvent);
    if (!res.success) {
      process.stderr.write(`[tv] ⚠️  stop event failed: ${res.stderr.trim()}\n`);
    }
    break;
  }

  case "tool-use": {
    const opts = parseArgs(args);
    const sessionId = getOrCreateSessionId();
    initLocalSession(sessionId);

    const toolUseId = opts.toolUseId ?? randomUUID();
    const isError = opts.isError === "true";
    const toolInput = opts.input ? JSON.parse(opts.input) : null;
    const toolResponse = opts.response ?? "";

    appendToolUseToTranscript(sessionId, opts.toolName, toolUseId, toolInput);
    appendToolResultToTranscript(sessionId, toolUseId, isError, toolResponse);

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
