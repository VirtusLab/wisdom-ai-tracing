#!/usr/bin/env node
/**
 * review-mcp — project-local MCP server for agent-driven code review.
 *
 * Exposes one tool:
 *
 *   agent_review — assembles the git diff for a range (with extra context)
 *                  plus the full content of the small touched files, and
 *                  returns them as a review prompt for the calling agent to
 *                  evaluate. Large files are left to their diff hunks so the
 *                  prompt stays within the agent's token budget.
 *
 *                  The agent reads the prompt, performs the review in its own
 *                  context, and responds with its findings. That response is
 *                  recorded by TraceVault as the tool result.
 *
 *                  TraceVault policy: RequiredToolCall on mcp__review__agent_review.
 *                  Action: Warn (not block) — review is advisory for now.
 *
 * The tool never calls an external LLM itself. The reviewing agent is the
 * same agent that has been working in the repo all session — it already has
 * full codebase context and is better placed to review than a cold LLM call.
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { spawn } from "child_process";
import { readFileSync, existsSync } from "fs";
import { resolve, relative, dirname, isAbsolute } from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "../..");

// Context limits — keep the assembled prompt within the agent's tool-result
// token cap. The diff (with extra surrounding context) is the primary review
// material; full file bodies are included only for small files and bounded by
// a total budget, so a change touching a large file (e.g. analytics.rs) no
// longer blows past the limit.
const DIFF_CONTEXT_LINES = 30;
const MAX_FILE_LINES = 400;
const MAX_TOTAL_CONTEXT = 64 * 1024;

/** Run a command synchronously, return stdout or throw on non-zero exit. */
function git(...args) {
  const result = spawn("git", args, { cwd: REPO_ROOT, shell: false });
  return new Promise((res, rej) => {
    let stdout = "";
    let stderr = "";
    result.stdout.on("data", (d) => (stdout += d.toString()));
    result.stderr.on("data", (d) => (stderr += d.toString()));
    result.on("close", (code) => {
      if (code === 0) res(stdout);
      else rej(new Error(`git ${args.join(" ")} failed: ${stderr.trim()}`));
    });
    result.on("error", (e) => rej(e));
  });
}

/**
 * Return the list of files modified in the given git range.
 * Uses --name-only --diff-filter=ACMR to skip deletions (no content to show).
 */
async function touchedFiles(range) {
  const out = await git(
    "diff",
    "--name-only",
    "--diff-filter=ACMR",
    range,
    "--"
  );
  return out
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}

/** Return the unified diff for the given range, with extra surrounding context. */
async function getDiff(range) {
  return git("diff", `-U${DIFF_CONTEXT_LINES}`, range, "--");
}

/** Read a file relative to repo root, return content or an error note. */
function readFile(relPath) {
  const abs = resolve(REPO_ROOT, relPath);
  // Guard against path traversal: resolved path must stay within REPO_ROOT
  const rel = relative(REPO_ROOT, abs);
  if (rel.startsWith("..") || isAbsolute(rel)) {
    return `(file path rejected: ${relPath})`;
  }
  if (!existsSync(abs)) return `(file not found: ${relPath})`;
  try {
    const content = readFileSync(abs, "utf8");
    // Cap individual files at 200 KB to avoid absurd token counts
    const MAX = 200 * 1024;
    if (content.length > MAX) {
      return (
        content.slice(0, MAX) +
        `\n\n... (truncated — file exceeds 200 KB, showing first ${MAX} bytes)`
      );
    }
    return content;
  } catch (e) {
    return `(could not read ${relPath}: ${e.message})`;
  }
}

/**
 * Build the "full content" section: include each touched file's full content
 * only when it is small (≤ MAX_FILE_LINES) and the running total stays under
 * MAX_TOTAL_CONTEXT. Larger / overflow files are noted by name and left to the
 * diff hunks (which carry DIFF_CONTEXT_LINES of surrounding context). This
 * keeps the prompt bounded regardless of how large the touched files are.
 */
function buildFileContext(files) {
  const sections = [];
  let used = 0;
  for (const f of files) {
    if (isAbsolute(f) || f.includes("..")) {
      sections.push(`### ${f}\n(file path rejected)`);
      continue;
    }
    const content = readFile(f);
    const lineCount = content.split("\n").length;
    if (lineCount > MAX_FILE_LINES) {
      sections.push(
        `### ${f}\n(large file — ${lineCount} lines; review via the diff hunks above)`
      );
      continue;
    }
    if (used + content.length > MAX_TOTAL_CONTEXT) {
      sections.push(
        `### ${f}\n(omitted — file-context budget reached; review via the diff hunks above)`
      );
      continue;
    }
    used += content.length;
    sections.push(`### ${f}\n\`\`\`\n${content}\n\`\`\``);
  }
  return sections.join("\n\n");
}

// ── MCP server ────────────────────────────────────────────────────────────────

const server = new McpServer({
  name: "review-mcp",
  version: "1.0.0",
});

server.tool(
  "agent_review",
  `Prepare a code review prompt for the calling agent to evaluate.

This tool assembles the git diff for a commit range (with extra surrounding
context) plus the full content of the SMALL touched files, then returns a
structured review prompt. Large files are reviewed via their diff hunks so the
prompt stays within the agent's token budget. The calling agent should read the
prompt, perform the review in its own context, and respond with its findings.

The tool result (the agent's review response) is recorded by TraceVault as
a tool call event. The TraceVault policy for this repo requires this tool to
be called before pushing — the policy action is Warn, so a push is never
blocked, but the review findings are always recorded in the session trace.

Parameters:
  git_range  — git revision range to review, e.g. "HEAD~3..HEAD" or
               "abc1234..def5678". Defaults to "HEAD~1..HEAD" (last commit).`,
  {
    git_range: z
      .string()
      .optional()
      .describe(
        'Git revision range to review, e.g. "HEAD~3..HEAD". Defaults to "HEAD~1..HEAD".'
      ),
  },
  async ({ git_range }) => {
    const range = git_range?.trim() || "HEAD~1..HEAD";

    let diff;
    try {
      diff = await getDiff(range);
    } catch (e) {
      return {
        content: [
          {
            type: "text",
            text: `❌ Could not compute diff for range "${range}": ${e.message}`,
          },
        ],
        isError: true,
      };
    }

    if (!diff.trim()) {
      return {
        content: [
          {
            type: "text",
            text: `No changes found in range "${range}". Nothing to review.`,
          },
        ],
      };
    }

    let files;
    try {
      files = await touchedFiles(range);
    } catch (e) {
      return {
        content: [
          {
            type: "text",
            text: `❌ Could not list touched files for range "${range}": ${e.message}`,
          },
        ],
        isError: true,
      };
    }

    // Full content for small files only, within a total budget; large files
    // are reviewed via the diff hunks (carrying extra surrounding context).
    const fileContext = buildFileContext(files);

    const prompt = `Please review the following changes critically. Make sure the code is idiomatic, and doesn't introduce bugs and vulnerabilities. Report any places the change can be improved. Focus on the code that changed; the diff below carries surrounding context, and full bodies of small touched files are appended for reference.

---

## Changes to review (${range})

\`\`\`diff
${diff}
\`\`\`

---

## Full content of small touched files (large files shown as diff only)

${fileContext || "(no files to show)"}

---

Provide your review findings. If everything looks good, say so clearly. If there are issues, list them specifically with file and line references where possible.`;

    return {
      content: [{ type: "text", text: prompt }],
    };
  }
);

// ── Start ─────────────────────────────────────────────────────────────────────

// Start the stdio server only when run directly, so tests can import the
// helpers without spawning the server.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

export { buildFileContext, getDiff, touchedFiles };
