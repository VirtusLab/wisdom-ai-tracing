# Visdom Trace

> **Research Preview** — This project is under heavy development. APIs, configuration, and features may change significantly between releases.

![Analytics Overview](docs/images/analytics-overview.png)

AI code governance platform for enterprises. Captures what AI coding agents do in your repos — which files they touch, how many tokens they burn, what tools they call, what percentage of code is AI-generated — then enforces policies and produces tamper-evident audit trails for regulatory compliance.

Built for financial institutions and regulated industries where AI-generated code needs the same audit rigor as human-written code.

[Learn more at VirtusLab](https://virtuslab.com/services/tracevault)

## Five Pillars of AI Governance

### 1. Capture — Full Session Tracing

Record every AI interaction with full fidelity. Visdom Trace hooks into AI coding agents and automatically captures session transcripts, token breakdowns (input/output/cache per model), every tool call invocation, file modifications with diffs, and cost estimates. Secrets and credentials are redacted before storage.

Nothing to configure — once initialized, capture is automatic and invisible.

### 2. Enforce — Policy Engine

Keep AI within bounds. Define rules per-repository that are evaluated on every push:

- **Model allowlists** — restrict which AI models can be used
- **Sensitive path protection** — flag AI edits to critical paths (`/payments/`, `/auth/`, `/crypto/`)
- **Required tool calls** — mandate security scanners or code review tools
- **AI percentage thresholds** — warn when AI-authored code exceeds a limit
- **Token budgets** — cap token usage or cost per session

Policies can either **block the push** (exit non-zero) or **warn**. Fail-closed by default: if the server is unreachable, the push is blocked.

### 3. Audit — Cryptographically Signed Chain of Events

Every trace pushed to the server is transformed into a tamper-proof record:

1. **Hashed** — SHA-256 digest of the canonical record
2. **Chained** — each hash links to the previous, forming a verifiable chain
3. **Signed** — Ed25519 digital signature proves authenticity
4. **Sealed** — timestamp marks when the record was finalized

Records are append-only — no updates, no deletes. Corrections create amendment records referencing the original. The entire chain can be verified at any time to prove nothing was altered or reordered.

Built-in compliance modes for **SOX** (7-year retention), **PCI-DSS** (1-year, WORM-equivalent), and **SR 11-7** (model risk management for banks). RBAC with five roles — including a dedicated **Auditor** role with read-only access to all traces and the audit log.

### 4. Analyze — Usage Analytics

Understand how AI is used across your team:

- **Token usage trends** over time, per model, per author
- **Model distribution** — which models are used most and where
- **Cost tracking** — estimated cost breakdown by model and team member
- **Cache savings** — how much prompt caching saves
- **AI attribution** — what percentage of your codebase is AI-generated
- **Author activity** — commits, tokens, and cost per developer

All available through the web dashboard with filterable time ranges and drill-down views.

### 5. Code — Stories & Documentation

See exactly what AI wrote, line by line. The code browser overlays AI attribution on your source files — highlighting which lines were AI-generated, which function or class they belong to (via tree-sitter scope detection), and linking back to the session that produced them.

**Story generation** turns raw traces into human-readable narratives: why the AI chose a particular pattern, what alternatives were considered, and what the developer's intent was. Auto-generated Architecture Decision Records give new team members full context without asking anyone.

## Architecture

Three Rust crates in a Cargo workspace:

- **tracevault-core** — domain types, policy engine (7 condition types), attribution engine (tree-sitter based), secret redactor
- **tracevault-cli** — CLI binary that hooks into Claude Code, captures traces locally, checks policies, pushes to server
- **tracevault-server** — axum HTTP server backed by PostgreSQL with Ed25519 signing, audit logging, RBAC, code browser

Plus a SvelteKit web dashboard and a GitHub Action for CI verification.

## Prerequisites

- Rust stable toolchain (install via [rustup](https://rustup.rs))
- PostgreSQL 14+ (or Docker) with the [pgvector](https://github.com/pgvector/pgvector) extension enabled (`CREATE EXTENSION vector;`). pgvector is available out of the box on all major managed providers (AWS RDS, GCP Cloud SQL, Azure Database for PostgreSQL, Supabase, Neon).
- Node.js 20+ and pnpm (for the web dashboard)
- Docker & Docker Compose (for containerized deployment)

## Quick Start

### 1. Build from source

```sh
cargo build --release
```

Binaries land in `target/release/`:
- `tracevault` (CLI)
- `tracevault-server`

### 2. Start with Docker Compose (recommended)

```sh
# Generate an encryption key (required — encrypts per-org signing keys in the DB)
export TRACEVAULT_ENCRYPTION_KEY=$(openssl rand -base64 32)

# Start all services
docker compose up -d --build
```

This starts three containers:

| Service | URL | Description |
|---------|-----|-------------|
| `db` | `localhost:5432` | PostgreSQL 16 |
| `server` | `localhost:3000` | Visdom Trace API server |
| `web` | `localhost:5173` | SvelteKit web dashboard |

Database migrations run automatically on startup. Open `http://localhost:5173` to access the dashboard.

To run just the database (useful during development):

```sh
docker compose up -d db
```

For the **Enterprise** edition (adds RAG chat, vector search, compliance features), see the [enterprise README](enterprise/README.md).

### 3. Run the backend server locally (development)

If you want to run the Rust server directly (instead of via Docker), start PostgreSQL first, then:

```sh
# Database credentials (defaults match docker-compose)
export DATABASE_URL=postgres://tracevault:tracevault@localhost:5432/tracevault

# Server bind address
export HOST=0.0.0.0
export PORT=3000

# Encryption key for per-org signing keys (see "Keys & Secrets" below)
export TRACEVAULT_ENCRYPTION_KEY=<base64-encoded-32-byte-key>

cargo run -p tracevault-server
```

Database migrations run automatically on startup.

### 4. Run the web dashboard

The frontend is a separate SvelteKit app that proxies API calls to the backend:

```sh
cd web
pnpm install
pnpm dev
```

The dashboard runs on `http://localhost:5173` and proxies API calls to `http://localhost:3000` by default. To point at a different backend, set `PUBLIC_API_URL`:

```sh
PUBLIC_API_URL=http://your-server:3000 pnpm dev
```

### 5. Run tests

Unit tests (no database required):

```sh
cargo test -p tracevault-core
cargo test -p tracevault-server --lib
cargo test -p tracevault-cli
```

Integration tests require a running PostgreSQL instance. A separate test database is provided via Docker Compose to avoid interfering with your development database:

```sh
# Start the test database (port 5433, uses tmpfs — no persistence)
docker compose -f docker-compose.test.yml up -d

# Run integration tests
source .env.test && cargo test -p tracevault-server --test '*'

# Tear down
docker compose -f docker-compose.test.yml down
```

The test database runs on port **5433** with separate credentials (`tracevault_test`), so it won't conflict with the dev database on port 5432.

### 6. Initialize Visdom Trace in a repository

```sh
cd /path/to/your/repo
tracevault init
```

This creates a `.tracevault/` directory and installs a pre-push hook. The hook runs:

```sh
tracevault sync       # sync repo metadata with server
tracevault check      # evaluate policies against server rules (blocks push on failure)
```

Session and commit data is streamed to the server continuously via the Claude Code hooks (`tracevault stream`) and the git post-commit hook (`tracevault commit-push`), so there is no separate upload step.

The command also installs the Claude Code hook configuration. By default it writes to `.claude/settings.json` (shared with the team). Pass `--claude-settings local` (or pick `local` at the interactive prompt) to install into `.claude/settings.local.json` instead — the personal, git-ignored variant. Each hook runs `tracevault stream --event <type>`, which records the event locally and pushes it to the server in real time:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "tracevault stream --event pre-tool-use",
            "timeout": 5
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Write|Edit|Bash",
        "hooks": [
          {
            "type": "command",
            "command": "tracevault stream --event post-tool-use",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

### 7. Authenticate

```sh
# Log in to a Visdom Trace server.
# Prints an auth URL and attempts to open it in your default browser.
# In Docker/CI/SSH-without-X11 the browser open is skipped automatically —
# copy the printed URL into a browser on your workstation. You can also
# force that behaviour with `--no-browser` or `TRACEVAULT_NO_BROWSER=1`.
tracevault login --server-url https://your-server.example.com

# Check policies before pushing (also runs automatically via pre-push hook):
tracevault check

# Retry any events that failed to stream in real time (network blip, server down):
tracevault flush

# View local session stats:
tracevault stats

# Verify commits are sealed on the server:
tracevault verify --range HEAD~5..HEAD
```

## Using with Claude Code

Visdom Trace is designed to work with [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (Anthropic's CLI for Claude). Here's how to get started:

### 1. Install Claude Code

```sh
npm install -g @anthropic-ai/claude-code
```

### 2. Log in to Claude Code

```sh
claude login
```

This opens a browser for authentication with your Anthropic account.

### 3. Initialize Visdom Trace in your repo

```sh
cd /path/to/your/repo
tracevault login --server-url https://your-tracevault-server.example.com
tracevault init
```

That's it. From this point on, every Claude Code session in this repo is automatically traced — tool calls, file edits, token usage, and model info are captured and streamed to the Visdom Trace server as they happen. When you `git push`, the pre-push hook evaluates policies and blocks the push if any rule fails.

### 4. Install project-local MCP tools

This repo ships project-local MCP servers that Claude Code uses to satisfy pre-push policies.

**Step 1 — Create your local `.mcp.json`:**

```sh
cp .mcp.json.example .mcp.json
```

`.mcp.json` is gitignored so it stays local. The example includes the three project servers needed for policy compliance. Add any private servers (deploy tools, DB access, etc.) to your local copy only.

**Step 2 — Install dependencies for the required policy tools:**

```sh
npm install --prefix tools/cargo-mcp
npm install --prefix tools/review-mcp
```

**What each server provides:**

**`tools/cargo-mcp/`** — required by pre-push policies:
- **`mcp__cargo__cargo_fmt`** — runs `cargo fmt`. Must be called before committing.
- **`mcp__cargo__cargo_check`** — runs `cargo clippy` then `cargo test`. Must be called and succeed before pushing.
- **`mcp__cargo__cargo_audit`** — runs `cargo audit`. Required when `Cargo.lock` changes.

**`tools/review-mcp/`** — required by the validation-scoped self-review policy:
- **`mcp__review__agent_review`** — assembles the diff + full context of touched files and returns a review prompt. Call this inside a validation window before pushing to Rust files.

**Optional — session history queries:**

```sh
npm install --prefix integrations/tracevault-mcp
```

Installs two tools:

- **`ask_tracevault`** — lets agents query indexed session history in natural language ("Why was this refactored?", "What sessions touched the auth service?"). Not required by any policy. Needs `tracevault login` once.
- **`agent_policies`** — returns the rendered policy instructions for the current repo (same output as `tracevault agent-policies`). Call this at session start so the agent's behaviour matches the configured policies.

Claude Code picks up all servers from `.mcp.json` automatically on next session start. No further configuration needed.

## Integrating Visdom Trace into your own project

When you run `tracevault init` in a repository, Visdom Trace installs the session hooks automatically. To make sure your AI agents understand the governance expectations, add a `CLAUDE.md` (or equivalent) entry pointing them at `tracevault agent-policies`.

### Recommended `CLAUDE.md` for a Visdom Trace–integrated project

```
# Visdom Trace session tracking

Before starting any session, run `tracevault agent-policies` (or call the
`agent_policies` MCP tool if the tracevault MCP is installed) and follow
the instructions it returns. Those instructions reflect the policies
configured on the server and take precedence over anything below.

## Commit messages

Use conventional commits: https://www.conventionalcommits.org/en/v1.0.0/
```

`tracevault agent-policies` fetches Markdown instructions rendered by the server from the active policies — which tools must be called before push, which must succeed, what file patterns trigger conditional checks, and how the validation window works. The same output is available via the `agent_policies` MCP tool and as a **Preview** button on each repo page in the dashboard. When policies change, the output updates automatically.

> **Tool installation:** Rendered instructions reference tools by their literal name (e.g. `mcp__cargo__cargo_fmt`). Make sure those tools are installed in your agent setup. See [Install project-local MCP tools](#4-install-project-local-mcp-tools) for the bundled tools shipped with this repo.

### Policy types you can configure

In the Visdom Trace dashboard, under **Repos → [your repo] → Policies**, you can add:

| Condition | What it checks | Use case |
|---|---|---|
| `RequiredToolCall` | A specific tool was called during the session | Mandate code formatters, linters, test runners |
| `ConditionalToolCall` | A tool was called when specific files changed | Require security scan only when `Cargo.lock` changes |
| `AiPercentageThreshold` | AI-authored lines exceed a threshold | Warn when AI writes > 80% of a module |
| `TokenBudget` | Token or cost usage exceeds a limit | Cap AI spend per session |

Actions: **Block push** (exit non-zero, prevents `git push`) or **Warn** (logs but allows).

Scope: **Session** (evaluate all tool calls in the session) or **Validation** (evaluate only tools called after `tracevault validation-start`).

## Keys & Secrets

### Encryption key (`TRACEVAULT_ENCRYPTION_KEY`)

**Required.** AES-256 encryption key used to encrypt sensitive data at rest — including per-org Ed25519 signing keys, deploy keys, and API keys stored in the database.

Generate a 32-byte key and base64-encode it:

```sh
# Using openssl:
openssl rand -base64 32

# Or using Python:
python3 -c "import secrets, base64; print(base64.b64encode(secrets.token_bytes(32)).decode())"
```

Set it as an environment variable:

```sh
export TRACEVAULT_ENCRYPTION_KEY=<output-from-above>
```

**Backup this key.** If lost, all encrypted data in the database (org signing keys, deploy keys, API keys) becomes unrecoverable. For production, store it in your secrets manager (Vault, AWS Secrets Manager, etc.) and inject it at deploy time.

### Per-org signing keys

Each organization gets its own Ed25519 signing key for the tamper-evident audit trail. These are **generated automatically** when an org is created (via the UI or API) and encrypted at rest using `TRACEVAULT_ENCRYPTION_KEY`. You can also provide your own key during org creation.

To export an org's public key (for external verification):

```
GET /api/v1/orgs/{id}/compliance/public-key
```

### Database credentials

Default credentials (matching `docker-compose.yml`):

```sh
export DATABASE_URL=postgres://tracevault:tracevault@localhost:5432/tracevault
```

For production, use strong credentials and TLS:

```sh
export DATABASE_URL=postgres://user:password@host:5432/tracevault?sslmode=require
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `tracevault init [--server-url URL] [--claude-settings shared\|local]` | Initialize Visdom Trace in current repo, install pre-push hook and Claude Code hooks. `--claude-settings` chooses between `.claude/settings.json` (default) and `.claude/settings.local.json`; prompts interactively if omitted on a TTY |
| `tracevault login --server-url URL [--no-browser]` | Authenticate via device auth flow. Prints the URL and opens a browser when possible; `--no-browser` (or a headless env) skips the auto-open. |
| `tracevault logout` | Clear local credentials |
| `tracevault stream --event <type>` | Handle a Claude Code hook event (reads JSON from stdin) and stream it to the server |
| `tracevault sync` | Sync repo metadata with the server |
| `tracevault check` | Evaluate policies against server rules, exit non-zero if blocked |
| `tracevault validation-start [--session-id ID]` | Open a validation window. Call this when work is complete and you are ready to run pre-push validation tools. Validation-scoped policies only evaluate tools called after this point. Calling it again invalidates the previous window. |
| `tracevault stats` | Show local session statistics |
| `tracevault verify` | Verify commits are registered and sealed on the server (`--commits` or `--range`) |
| `tracevault status` | Show current session status (not yet implemented) |

## Policy Enforcement

Policies are managed per-repo through the web UI or API. Seven condition types:

| Condition | What it checks |
|-----------|---------------|
| Trace Completeness | Every commit has a corresponding trace |
| AI Percentage Threshold | Warns when AI-authored code exceeds a threshold |
| Model Allowlist | Only approved model families allowed |
| Sensitive Path Pattern | Flags AI edits to sensitive paths (payments, auth, crypto) |
| Required Tool Call | Ensures specific tools were called during the session |
| Conditional Tool Call | Requires a tool when files matching glob patterns are modified |
| Token Budget | Warns when token usage or cost exceeds limits |

Each policy has a configurable action:
- **Block Push** — `tracevault check` exits non-zero, preventing `git push`
- **Warn** — prints a warning but allows the push

Fail-closed: if the server is unreachable, `tracevault check` blocks the push.

### Validation Scope

A validation window is a pre-push gate phase. When work is complete and the agent is ready to push, it opens a validation window with `tracevault validation-start`, then runs all tools required by Validation-scoped policies (formatters, linters, security scanners, code review tools). Only tools explicitly allowed by those policies should be called between the window opening and the push.

If a validation tool fails and the agent needs to make a fix, it must restart the window (`tracevault validation-start` again) before rerunning the tools — the previous window is invalidated and all expected tools must be called again from scratch.

This ensures the audit trail proves that every required quality gate was run against the final state of the code, not just at some earlier point in a long session.

## Compliance & Audit Trail

### Compliance Modes

| Mode | Min Retention | Signing | Chain Verification | Use Case |
|------|--------------|---------|-------------------|----------|
| SOX | 7 years | Required | Daily | Public companies, financial reporting systems |
| PCI-DSS | 1 year | Required | Daily | Payment processing, cardholder data environments |
| SR 11-7 | 3 years | Required | Weekly | Banks using AI models in risk/trading/compliance |
| Custom | Configurable | Configurable | Configurable | Mix-and-match for specific requirements |

Compliance mode is set per-organization. When active, the system enforces minimum retention periods and required signing — admins can increase but not decrease below framework minimums.

### RBAC (Role-Based Access Control)

Five roles enforce separation of duties:

| Role | Push Traces | Manage Policies | View All Traces | View Audit Log | Manage Users |
|------|------------|----------------|-----------------|---------------|-------------|
| Owner | Yes | Yes | Yes | Yes | Yes |
| Admin | Yes | Yes | Yes | Yes | Yes |
| Policy Admin | Yes | Yes | Yes | Yes | No |
| Developer | Yes | No | Own only | No | No |
| Auditor | No | No | Yes (read-only) | Yes (read-only) | No |

### Audit Log

Every state-changing operation is logged to an append-only audit trail:
- Trace creation and sealing
- Policy CRUD and check results
- User login/logout and registration
- Role changes
- Compliance settings updates
- Chain verification results

Query via API with filters (action type, actor, resource, date range) or browse in the web dashboard.

## GitHub Action

Add to your workflow to verify that commits in a PR or push have corresponding traces sealed on the server:

```yaml
- uses: softwaremill/tracevault@main
  with:
    server-url: https://your-tracevault-server.example.com
    api-key: ${{ secrets.TRACEVAULT_API_KEY }}
```

The action installs the Visdom Trace CLI, detects the commit range from the PR or push event, runs `tracevault verify --range`, and writes a pass/fail summary to the GitHub Actions step summary.

## Configuration

### CLI config (`.tracevault/config.toml`)

```toml
agent = "claude-code"
# server_url = "https://your-server.example.com"
# api_key = "your-api-key"
```

### Server environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgres://tracevault:tracevault@localhost:5432/tracevault` | PostgreSQL connection string |
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `3000` | Bind port |
| `CORS_ORIGIN` | _(permissive)_ | Allowed CORS origin for web dashboard |
| `TRACEVAULT_ENCRYPTION_KEY` | — | **Required.** AES-256 encryption key (base64-encoded 32 bytes) for encrypting per-org signing keys, deploy keys, and API keys at rest. Generate with `openssl rand -base64 32`. |
| `TRACEVAULT_REPOS_DIR` | `./data/repos` | Directory for cloned git repos (used by code browser) |
| `TRACEVAULT_LLM_PROVIDER` | — | LLM provider for story generation (`anthropic` or `openai`) |
| `TRACEVAULT_LLM_API_KEY` | — | API key for the LLM provider |
| `TRACEVAULT_LLM_MODEL` | — | LLM model name (defaults: Claude Sonnet 4 for Anthropic, GPT-4o for OpenAI) |
| `TRACEVAULT_LLM_BASE_URL` | — | Custom LLM endpoint URL |
| `RUST_LOG` | — | Log level (e.g. `info`, `debug`) |

### Web dashboard environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PUBLIC_API_URL` | `http://localhost:3000` | Backend server URL the SvelteKit proxy forwards API calls to. In Docker Compose this is set to `http://server:3000` automatically. |

## License

Apache-2.0
