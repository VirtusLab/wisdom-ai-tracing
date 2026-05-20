# TraceVault GSD2 Ecosystem Tracker

A GSD2 ecosystem extension that streams GSD2 session activity to TraceVault as
Claude Code-compatible events. Lets GSD2 sessions appear in TraceVault's analytics,
session detail view, and chat RAG — without waiting for the full GSD2 adapter (PR #149).

## How it works

Hooks into GSD2's extension system (`session_start`, `tool_execution_end`,
`agent_end`, `stop`) and POSTs events to TraceVault using Claude Code protocol v1.
The server handles them identically to real CC sessions.

## Installation

```bash
# 1. Copy the extension into your project's .gsd/extensions/
cp tracevault-tracker.js /path/to/your/project/.gsd/extensions/

# 2. Trust the project (required by GSD's ecosystem loader)
# Either run `pi trust` in the project, or add manually:
node -e "
  const fs = require('fs');
  const p = require('os').homedir() + '/.gsd/agent/trusted-projects.json';
  const projects = fs.existsSync(p) ? JSON.parse(fs.readFileSync(p)) : [];
  const projectPath = require('path').resolve('.');
  if (!projects.includes(projectPath)) {
    projects.push(projectPath);
    fs.writeFileSync(p, JSON.stringify(projects, null, 2));
    console.log('Trusted:', projectPath);
  }
"

# 3. Ensure credentials exist
# Run: tracevault login
# This creates ~/.config/tracevault/credentials.json
```

## Configuration

Edit the top of `tracevault-tracker.js` to set your org and repo:

```js
const ORG_SLUG = "your-org-slug";
const REPO_ID  = "your-repo-uuid";
```

The repo must already be registered in TraceVault (`tracevault init` in the project).

## What gets tracked

- Session start/end with duration and final token stats
- Every tool call (name, input, output)
- Token usage per turn (input/output/cache) attributed to the correct model
- File changes extracted server-side from Write/Edit tool calls

## Requirements

- GSD2 (pi/GSD-2) with ecosystem extensions enabled
- TraceVault CLI logged in (`tracevault login`)
- Project registered in TraceVault (`tracevault init`)
