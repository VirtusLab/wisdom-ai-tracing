// Smoke test for review-mcp's prompt assembly: large files must NOT be inlined
// (only noted), small files are included, and the section stays bounded.
// Run: node tools/review-mcp/test.mjs
import assert from "node:assert/strict";
import { buildFileContext } from "./index.js";

const out = buildFileContext([
  "crates/tracevault-server/src/api/analytics.rs", // large (~2160 lines)
  "Cargo.toml", // small
]);

// Large file: noted by line count, NOT inlined as a fenced body.
assert.match(
  out,
  /analytics\.rs\n\(large file — \d+ lines/,
  "large file should be noted, not inlined"
);

// Small file: full content present (Cargo.toml contains [workspace]).
assert.match(
  out,
  /Cargo\.toml\n```[\s\S]*\[workspace\]/,
  "small file full content should be included"
);

// Bounded: the whole section is far smaller than analytics.rs alone (~80 KB).
assert.ok(
  out.length < 64 * 1024,
  `file-context section should be bounded, got ${out.length} bytes`
);

// Path traversal still rejected.
assert.match(
  buildFileContext(["../etc/passwd"]),
  /file path rejected/,
  "path traversal should be rejected"
);

console.log(
  `OK — file-context bounded at ${out.length} bytes; large file omitted, small file included`
);
