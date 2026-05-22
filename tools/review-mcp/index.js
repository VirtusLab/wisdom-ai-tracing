#!/usr/bin/env node
/**
 * review-mcp — project-local MCP server for agent-driven code review.
 *
 * Exposes one tool:
 *
 *   agent_review — assembles the git diff for a range plus the full content
 *                  of every file touched by that range, and returns them as
 *                  a review prompt for the calling agent to evaluate.
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
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "../..");

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

/** Return the unified diff for the given range. */
async function getDiff(range) {
  return git("diff", range, "--");
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

// ── MCP server ────────────────────────────────────────────────────────────────

const server = new McpServer({
  name: "review-mcp",
  version: "1.0.0",
});

server.tool(
  "agent_review",
  `Prepare a code review prompt for the calling agent to evaluate.

This tool assembles the git diff for a commit range together with the full
content of every file touched by that range, then returns a structured review
prompt. The calling agent should read the prompt, perform the review in its
own context, and respond with its findings.

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

    // Build the file context section
    // Filter out any paths that would escape the repo root before reading
    const fileContext = files
      .map((f) => {
        if (isAbsolute(f) || f.includes("..")) {
          return `### ${f}\n\`\`\`\n(file path rejected)\n\`\`\``;
        }
        const content = readFile(f);
        return `### ${f}\n\`\`\`\n${content}\n\`\`\``;
      })
      .join("\n\n");

    const prompt = `Please review the following changes critically. Make sure the code is idiomatic, and doesn't introduce bugs and vulnerabilities. Report any places the change can be improved. Please focus on the code that changed — the full file content is provided only for context.

---

## Changes to review (${range})

\`\`\`diff
${diff}
\`\`\`

---

## Full content of touched files (for context only)

${fileContext || "(no files to show)"}

---

Provide your review findings. If everything looks good, say so clearly. If there are issues, list them specifically with file and line references where possible.`;

    return {
      content: [{ type: "text", text: prompt }],
    };
  }
);

// ── Start ─────────────────────────────────────────────────────────────────────

const transport = new StdioServerTransport();
await server.connect(transport);
