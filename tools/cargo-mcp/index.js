#!/usr/bin/env node
/**
 * cargo-mcp — project-local MCP server for TraceVault.
 *
 * Exposes two tools for use with AI coding agents:
 *
 *   cargo_fmt  — runs `cargo fmt` to format all Rust files in place.
 *                TraceVault policy: require this tool to be called (any result).
 *                Rationale: the agent must format the code before committing.
 *
 *   cargo_check — runs `cargo clippy` then `cargo test`.
 *                Returns isError=true when either step fails so TraceVault's
 *                must_succeed policy can distinguish "ran but failed" from
 *                "ran and passed".
 *                TraceVault policy: require this tool to be called AND must_succeed=true.
 *                Rationale: enforces that linting and tests actually pass before pushing.
 *
 * Both tools run in the repo root (parent of tools/cargo-mcp/).
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { spawn } from "child_process";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
// Repo root is two levels up from tools/cargo-mcp/
const REPO_ROOT = resolve(__dirname, "../..");

/**
 * Run a command, capturing stdout+stderr.
 * Resolves with { code, stdout, stderr }.
 */
function run(cmd, args, cwd) {
  return new Promise((resolve) => {
    const proc = spawn(cmd, args, { cwd, shell: false });
    let stdout = "";
    let stderr = "";
    proc.stdout.on("data", (d) => (stdout += d.toString()));
    proc.stderr.on("data", (d) => (stderr += d.toString()));
    proc.on("close", (code) => resolve({ code: code ?? 1, stdout, stderr }));
    proc.on("error", (err) =>
      resolve({ code: 1, stdout: "", stderr: err.message })
    );
  });
}

/**
 * Build an MCP tool response object.
 * @param {string} text - Human-readable result message.
 * @param {boolean} isError - When true, sets the MCP isError flag so TraceVault
 *   records this call as failed (used by must_succeed policies).
 * @returns {{ content: Array<{type: string, text: string}>, isError?: true }}
 */
function buildMcpResponse(text, isError = false) {
  return {
    content: [{ type: "text", text }],
    ...(isError ? { isError: true } : {}),
  };
}

// ── MCP server ────────────────────────────────────────────────────────────────

const server = new McpServer({
  name: "cargo-mcp",
  version: "1.0.0",
});

// ── Tool: cargo_fmt ───────────────────────────────────────────────────────────

server.tool(
  "cargo_fmt",
  `Run \`cargo fmt\` on the repository to format all Rust source files in place.

Call this tool before committing to ensure code is properly formatted. The TraceVault
policy for this repo requires that you call this tool at least once per session.
Fails only if cargo itself cannot run (e.g. toolchain not installed).`,
  {},
  async () => {
    const { code, stdout, stderr } = await run(
      "cargo",
      ["fmt"],
      REPO_ROOT
    );

    if (code === 0) {
      return buildMcpResponse("✅ Code formatted successfully.");
    }

    const details = [stderr, stdout].filter(Boolean).join("\n").trim();
    return buildMcpResponse(`❌ cargo fmt failed unexpectedly.\n\n${details}`, true);
  }
);

// ── Tool: cargo_check ─────────────────────────────────────────────────────────

server.tool(
  "cargo_check",
  `Run \`cargo clippy\` and unit tests (\`cargo test --lib --bins\`) on the repository.
Both steps must pass. Integration tests that require a live database are excluded.

Returns isError=true if either clippy or tests fail, which TraceVault's must_succeed
policy uses to determine whether quality gates were actually met (not just invoked).

Call this tool before pushing. The TraceVault policy for this repo requires this tool
to be called AND to succeed (must_succeed=true). Fix any clippy warnings or test
failures before pushing.`,
  {},
  async () => {
    // Step 1: clippy
    const clippy = await run(
      "cargo",
      ["clippy", "--all-targets", "--", "-D", "warnings"],
      REPO_ROOT
    );

    if (clippy.code !== 0) {
      const details = [clippy.stderr, clippy.stdout].filter(Boolean).join("\n").trim();
      return buildMcpResponse(
        `❌ cargo clippy failed — fix warnings before pushing.\n\n${details}`,
        true // isError=true → TraceVault marks this as a failed call
      );
    }

    // Step 2: unit tests (--lib excludes integration tests that require a live DB)
    const test = await run(
      "cargo",
      ["test", "--lib", "--bins", "-p", "tracevault-core", "-p", "tracevault-cli", "-p", "tracevault-server"],
      REPO_ROOT
    );

    if (test.code !== 0) {
      const details = [test.stderr, test.stdout].filter(Boolean).join("\n").trim();
      return buildMcpResponse(
        `❌ cargo test failed — fix failing tests before pushing.\n\n${details}`,
        true // isError=true → TraceVault marks this as a failed call
      );
    }

    const summary = test.stdout
      .split("\n")
      .filter((l) => l.includes("test result") || l.includes("running"))
      .join("\n")
      .trim();

    return buildMcpResponse(
      `✅ clippy and tests passed.\n\n${summary || "All tests OK."}`
    );
  }
);

// ── Start ─────────────────────────────────────────────────────────────────────

const transport = new StdioServerTransport();
await server.connect(transport);
