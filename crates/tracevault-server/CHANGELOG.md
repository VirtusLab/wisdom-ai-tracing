# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.16.1](https://github.com/softwaremill/tracevault/compare/v0.16.0...v0.16.1) - 2026-06-01

### Added

- agent-policies — server-rendered policy instructions for agents
- *(branches)* add Repo column to branches view
- *(sessions)* add tool, user, and file changes filters
- *(ui)* multi-select filters for policy activity table
- *(mcp)* add TraceVault MCP server and stateless chat/ask endpoint
- *(ui)* add token and cost breakdown split to author detail stats
- *(policies)* persist validation_window_gate evaluations to policy_evaluations
- *(policies)* validation window for scoped policy enforcement
- *(policies)* add must_succeed flag to tool call policies
- *(ui)* add server-side pagination to policy evaluations table
- *(orgs)* allow renaming org slug and match it case-insensitively
- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- extract row mapper helper and copy-label derived
- address review feedback on PR #200
- eliminate WHERE clause duplication and fix repo detail commit pagination
- extract all inline SQL in traces_ui submodules to sql/ files
- split traces_ui.rs into focused submodules
- *(server)* extract author detail stats query to sql file
- *(policies)* replace magic-name check with is_synthetic flag on policy_evaluations
- *(policies)* extract inline SQL queries to sql/ files
- *(policies)* address nit comments
- address review comments
- *(server)* extract count_evaluations SQL to .sql file
- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Documentation

- rename product TraceVault → Visdom Trace across all documentation

### Fixed

- *(server,cli)* default session tool to 'claude-code' to fix validation-start 500
- update stale comment about 'both' scope after removal
- *(policies)* remove 'both' scope, rename 'Window' to 'Validation'
- *(tokens)* store raw input_tokens, compute fresh only for pricing
- prevent memberless ghost orgs in login dropdown
- *(sso)* update comments to reflect owner-or-admin access
- *(sso)* allow admin role to manage SSO configuration
- *(ui)* add token breakdown to session detail page
- *(ui)* show actual total tokens + breakdown in sessions views
- address review agent findings in traces_ui
- *(pagination)* convert sessions and commits to server-side pagination
- *(mcp)* address review comments
- address review comments and failing permission count tests
- *(permissions)* grant ChatUse permission to owner, admin, developer roles
- *(tests)* update insert_evaluation calls for Option<Uuid> + is_synthetic params
- *(tests)* supply required tool field in window stats test sessions
- *(policies)* include skip in worst-result severity ordering
- *(policies)* address self-review findings
- *(policies)* address review comments
- *(ci)* fix rustfmt indentation in repo_policies_test
- *(auth)* move device status poll off strict rate limiter and handle 429 in CLI
- *(tests)* add missing tool_is_error field to InsertToolEvent in server tests
- *(server)* fix rustfmt indentation in count_evaluations
- *(security)* autofix Path traversal attack possible
- *(auth)* disable public registration once first user exists
- *(server)* remove unused Arc import that breaks enterprise build
- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.16.0](https://github.com/softwaremill/tracevault/compare/v0.15.0...v0.16.0) - 2026-05-27

### Added

- agent-policies — server-rendered policy instructions for agents
- *(branches)* add Repo column to branches view
- *(sessions)* add tool, user, and file changes filters
- *(ui)* multi-select filters for policy activity table
- *(mcp)* add TraceVault MCP server and stateless chat/ask endpoint
- *(ui)* add token and cost breakdown split to author detail stats
- *(policies)* persist validation_window_gate evaluations to policy_evaluations
- *(policies)* validation window for scoped policy enforcement
- *(policies)* add must_succeed flag to tool call policies
- *(ui)* add server-side pagination to policy evaluations table
- *(orgs)* allow renaming org slug and match it case-insensitively
- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- extract row mapper helper and copy-label derived
- address review feedback on PR #200
- eliminate WHERE clause duplication and fix repo detail commit pagination
- extract all inline SQL in traces_ui submodules to sql/ files
- split traces_ui.rs into focused submodules
- *(server)* extract author detail stats query to sql file
- *(policies)* replace magic-name check with is_synthetic flag on policy_evaluations
- *(policies)* extract inline SQL queries to sql/ files
- *(policies)* address nit comments
- address review comments
- *(server)* extract count_evaluations SQL to .sql file
- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Documentation

- rename product TraceVault → Visdom Trace across all documentation

### Fixed

- update stale comment about 'both' scope after removal
- *(policies)* remove 'both' scope, rename 'Window' to 'Validation'
- *(tokens)* store raw input_tokens, compute fresh only for pricing
- prevent memberless ghost orgs in login dropdown
- *(sso)* update comments to reflect owner-or-admin access
- *(sso)* allow admin role to manage SSO configuration
- *(ui)* add token breakdown to session detail page
- *(ui)* show actual total tokens + breakdown in sessions views
- address review agent findings in traces_ui
- *(pagination)* convert sessions and commits to server-side pagination
- *(mcp)* address review comments
- address review comments and failing permission count tests
- *(permissions)* grant ChatUse permission to owner, admin, developer roles
- *(tests)* update insert_evaluation calls for Option<Uuid> + is_synthetic params
- *(tests)* supply required tool field in window stats test sessions
- *(policies)* include skip in worst-result severity ordering
- *(policies)* address self-review findings
- *(policies)* address review comments
- *(ci)* fix rustfmt indentation in repo_policies_test
- *(auth)* move device status poll off strict rate limiter and handle 429 in CLI
- *(tests)* add missing tool_is_error field to InsertToolEvent in server tests
- *(server)* fix rustfmt indentation in count_evaluations
- *(security)* autofix Path traversal attack possible
- *(auth)* disable public registration once first user exists
- *(server)* remove unused Arc import that breaks enterprise build
- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.15.0](https://github.com/softwaremill/tracevault/compare/v0.14.0...v0.15.0) - 2026-05-22

### Added

- *(ui)* add token and cost breakdown split to author detail stats
- *(policies)* persist validation_window_gate evaluations to policy_evaluations
- *(policies)* validation window for scoped policy enforcement
- *(policies)* add must_succeed flag to tool call policies
- *(ui)* add server-side pagination to policy evaluations table
- *(orgs)* allow renaming org slug and match it case-insensitively
- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(server)* extract author detail stats query to sql file
- *(policies)* replace magic-name check with is_synthetic flag on policy_evaluations
- *(policies)* extract inline SQL queries to sql/ files
- *(policies)* address nit comments
- address review comments
- *(server)* extract count_evaluations SQL to .sql file
- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Documentation

- rename product TraceVault → Visdom Trace across all documentation

### Fixed

- *(tests)* update insert_evaluation calls for Option<Uuid> + is_synthetic params
- *(tests)* supply required tool field in window stats test sessions
- *(policies)* include skip in worst-result severity ordering
- *(policies)* address self-review findings
- *(policies)* address review comments
- *(ci)* fix rustfmt indentation in repo_policies_test
- *(auth)* move device status poll off strict rate limiter and handle 429 in CLI
- *(tests)* add missing tool_is_error field to InsertToolEvent in server tests
- *(server)* fix rustfmt indentation in count_evaluations
- *(security)* autofix Path traversal attack possible
- *(auth)* disable public registration once first user exists
- *(server)* remove unused Arc import that breaks enterprise build
- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.14.0](https://github.com/softwaremill/tracevault/compare/v0.13.0...v0.14.0) - 2026-05-22

### Added

- *(policies)* persist validation_window_gate evaluations to policy_evaluations
- *(policies)* validation window for scoped policy enforcement
- *(policies)* add must_succeed flag to tool call policies
- *(ui)* add server-side pagination to policy evaluations table
- *(orgs)* allow renaming org slug and match it case-insensitively
- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(policies)* replace magic-name check with is_synthetic flag on policy_evaluations
- *(policies)* extract inline SQL queries to sql/ files
- *(policies)* address nit comments
- address review comments
- *(server)* extract count_evaluations SQL to .sql file
- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(tests)* update insert_evaluation calls for Option<Uuid> + is_synthetic params
- *(tests)* supply required tool field in window stats test sessions
- *(policies)* include skip in worst-result severity ordering
- *(policies)* address self-review findings
- *(policies)* address review comments
- *(ci)* fix rustfmt indentation in repo_policies_test
- *(auth)* move device status poll off strict rate limiter and handle 429 in CLI
- *(tests)* add missing tool_is_error field to InsertToolEvent in server tests
- *(server)* fix rustfmt indentation in count_evaluations
- *(security)* autofix Path traversal attack possible
- *(auth)* disable public registration once first user exists
- *(server)* remove unused Arc import that breaks enterprise build
- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.13.0](https://github.com/softwaremill/tracevault/compare/v0.12.0...v0.13.0) - 2026-05-21

### Added

- *(policies)* add must_succeed flag to tool call policies
- *(ui)* add server-side pagination to policy evaluations table
- *(orgs)* allow renaming org slug and match it case-insensitively
- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- address review comments
- *(server)* extract count_evaluations SQL to .sql file
- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(auth)* move device status poll off strict rate limiter and handle 429 in CLI
- *(tests)* add missing tool_is_error field to InsertToolEvent in server tests
- *(server)* fix rustfmt indentation in count_evaluations
- *(security)* autofix Path traversal attack possible
- *(auth)* disable public registration once first user exists
- *(server)* remove unused Arc import that breaks enterprise build
- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.12.0](https://github.com/softwaremill/tracevault/compare/v0.11.3...v0.12.0) - 2026-05-08

### Added

- *(orgs)* allow renaming org slug and match it case-insensitively
- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(security)* autofix Path traversal attack possible
- *(auth)* disable public registration once first user exists
- *(server)* remove unused Arc import that breaks enterprise build
- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.11.3](https://github.com/softwaremill/tracevault/compare/v0.11.2...v0.11.3) - 2026-04-23

### Added

- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(server)* remove unused Arc import that breaks enterprise build
- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.11.2](https://github.com/softwaremill/tracevault/compare/v0.11.1...v0.11.2) - 2026-04-23

### Added

- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.11.1](https://github.com/softwaremill/tracevault/compare/v0.11.0...v0.11.1) - 2026-04-23

### Added

- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.11.0](https://github.com/softwaremill/tracevault/compare/v0.10.0...v0.11.0) - 2026-04-23

### Added

- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.10.0](https://github.com/softwaremill/tracevault/compare/v0.9.0...v0.10.0) - 2026-04-22

### Added

- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add coverage before dep upgrades (rand, reqwest, tower_governor)
- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.9.0](https://github.com/softwaremill/tracevault/compare/v0.8.0...v0.9.0) - 2026-04-22

### Added

- *(login)* replace org text input with dropdown and SSO-first UX
- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC
- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(invites)* route existing accounts to login on invite accept
- *(chat)* dedupe referenced commits across matched sessions
- resolve clippy warnings in chat_indexing and story
- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.8.0](https://github.com/softwaremill/tracevault/compare/v0.7.0...v0.8.0) - 2026-04-14

### Added

- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(chat-indexing)* cap embedding batch size to prevent OOM
- replace nightly floor_char_boundary with stable helper
- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.7.0](https://github.com/softwaremill/tracevault/compare/v0.6.2...v0.7.0) - 2026-04-09

### Added

- add detailed logging to chat indexing and summarization pipeline
- auto-reindex sessions when transcript grows beyond indexed chunk count
- accept mention overrides in chat message endpoint
- add GET /chat/mentions endpoint for @ autocomplete
- add chat API endpoints with conversation CRUD and RAG message handler
- add chat query pipeline with filter extraction, two-stage retrieval, and response generation
- add chat indexing pipeline with session completion hook and backfill
- add chat conversations and messages repo layer
- add session summarization service for RAG indexing
- add transcript chunking logic with sliding windows for RAG
- add chat summarization LLM settings endpoints
- add fastembed wrapper service for local text embedding
- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps
- add migration for chat RAG tables, pgvector, and summarization config
- enrich stories with session transcripts and clickable references
- enrich story with session transcripts and clickable references
- add GET /code/sessions endpoint for function sessions
- add gather_function_sessions for code sessions endpoint
- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- simplify backfill to rely solely on chunk count comparison
- extract SessionSearchFilter struct to fix clippy too_many_arguments
- make resolve_org_llm public for chat module reuse
- replace regex session linkification with structured Linked Sessions section
- extract collect_file_commit_shas helper from story context
- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- use div_ceil instead of manual reimplementation
- exclude sessions without transcripts from indexing status and add backfill diagnostics
- segment large transcripts for summarization and fix byte-boundary panic
- use floor_char_boundary for string truncation to avoid panics on multi-byte chars
- index all sessions regardless of status in chat backfill
- grant admin role to API key auth instead of developer
- chat UX improvements and bug fixes
- make pgvector migration conditional for community installs without the extension
- fallback to file_path query when git SHAs miss DB sessions
- query sessions by file_path instead of git-walking
- unify password minimum length to 10 characters
- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.6.2](https://github.com/softwaremill/tracevault/compare/v0.6.1...v0.6.2) - 2026-04-01

### Added

- unify transcript display with timestamps across traces and analytics views
- report attributed_sessions_sealed in CI verification
- include session seal verification in chain verification
- background sweep to seal stale sessions after 30min inactivity
- seal sessions on SessionEnd when signing is enabled
- seal commits on push when signing is enabled
- add SealingService with commit and session sealing logic
- add sealing repo layer for commit and session seals
- migration for multi-seal sessions and verification counts
- add sign method to SigningService
- revamp invite flow UI, add invite acceptance page, improve error handling
- wire invite routes, remove old invite_member endpoint
- add invite link handlers (create, list, revoke, details, accept)
- add generate_invite_token utility
- add invite_expiry_minutes and cors_origin to AppState
- add org_invites migration for invite link flow
- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- make is_valid_email public for reuse in invite handler
- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- force-fetch refspecs and normalize deploy key PEM newline
- add sealed_at to session hash, fix chain race condition with advisory lock
- compute avg session duration from timestamps when duration_ms is missing
- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Security

- implement NIST 800-63B password policy with breached password check
- add email format validation on registration
- add per-IP rate limiting on public and auth endpoints
- add HMAC-SHA256 verification for GitHub webhook signatures
- require CORS_ORIGIN env var, remove permissive fallback
- sanitize Sqlx and Git error responses to prevent info leakage

### Test

- add integration tests for invite link flow
- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.6.1](https://github.com/softwaremill/tracevault/compare/v0.6.0...v0.6.1) - 2026-03-29

### Added

- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- remove needless borrows in encryption tests (clippy)
- resolve CI failures — add dead_code allows and avoid CodeQL hard-coded password flag
- use DB pricing rates during ingestion instead of hardcoded fallbacks
- prevent duplicate token/cost accumulation from overlapping transcript batches
- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Test

- add Tier 2 repo layer integration tests (api_keys, commits, policies, pricing, repos)
- add API layer unit tests (policies, dashboard, pagination)
- add server unit tests (auth, encryption, error, permissions, signing, pricing, config, attribution, stream)
- add repository layer integration tests

## [0.6.0](https://github.com/softwaremill/tracevault/compare/v0.5.0...v0.6.0) - 2026-03-29

### Added

- add keyset pagination types and rewrite IN-subqueries
- add hook adapter architecture with multi-tool detection
- add SQL indexes, materialized views, and tool field migration
- add repository layer for analytics, policies, compliance, pricing, code_stories
- add repository layer for sessions, events, commits
- add repository layer for users, orgs, repos, api_keys
- add AppError type with IntoResponse and permission helper
- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- migrate all handlers to AppError and repo/service layers
- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

### Test

- add repository layer integration tests

## [0.5.0](https://github.com/softwaremill/tracevault/compare/v0.4.0...v0.5.0) - 2026-03-28

### Added

- add top AI tools section to author detail page and fix DataTable clickability
- register AI tools analytics routes
- add AI tools analytics endpoints and software summary
- add AI tool usage tracking (migration + extraction)
- add top authors leaderboard to dashboard
- add author detail endpoint
- add user_id to AuthorLeaderboard, drop unused fields
- register software analytics routes
- add software user detail endpoint
- add software analytics list endpoint
- extract software usage from Bash events at ingest
- add user_software_usage migration
- add manual pricing sync endpoint and sync status endpoint
- wire startup + daily background pricing sync
- add sync_pricing function with diff, update, and recalculation
- add LiteLLM JSON parsing with model mapping and tests
- add source field to PricingEntry struct and queries
- add pricing sync migration (source column + sync log table)
- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- strip non-software data from software user detail endpoint
- slim get_software to org-wide tools only
- remove git-ai, compute attribution server-side from sessions
- clean up v2 references in comments, remove dead code and legacy script
- extract seal fields into commit_seals table in compliance and CI
- extract seal fields into session_seals in dashboard compliance query
- rename sessions_v2/commits_v2 in analytics
- rename sessions_v2/commits_v2 in remaining files
- rename sessions_v2/commits_v2 in traces_ui
- rename sessions_v2/commits_v2 in stream and commit_push endpoints
- consolidate migrations, remove v2 suffixes from schema
- use real session model names for pricing instead of canonical names
- extract shared recalculate_sessions_for_pricing function
- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- apply repo/author filters to AI summary and filter empty sessions
- apply repo/author filters to software analytics query
- cast SUM(total_tokens) to BIGINT in software user detail query
- resolve TypeScript narrowing errors in software pages
- use git CLI for clone/fetch to support all SSH key formats
- update migration 008 to use renamed sessions table
- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

## [0.4.0](https://github.com/softwaremill/tracevault/compare/v0.3.2...v0.4.0) - 2026-03-25

### Added

- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- populate tool frequency data in session analytics
- compute duration and messages from fallback sources in analytics sessions
- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

## [0.3.2](https://github.com/softwaremill/tracevault/compare/v0.3.1...v0.3.2) - 2026-03-25

### Added

- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- resolve clippy warning in startup sync
- auto-sync repos on startup, improve attribution blame and error UX
- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

## [0.3.1](https://github.com/softwaremill/tracevault/compare/v0.3.0...v0.3.1) - 2026-03-25

### Added

- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- fix attribution confidence scoring and deduplicate file changes
- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

## [0.3.0](https://github.com/softwaremill/tracevault/compare/v0.2.0...v0.3.0) - 2026-03-25

### Added

- *(stream)* extract token usage and costs from transcript chunks in real-time
- *(api)* add traces UI endpoints (sessions, commits, timeline, attribution, branches) and remove old traces module
- *(branch-tracking)* track commits reaching branches and tags via webhooks
- *(attribution)* add line-level attribution engine with confidence scoring
- *(api)* add streaming event endpoint
- *(api)* add commit push endpoint with file-level attribution
- *(schema)* add streaming architecture tables
- *(pricing)* add pricing CRUD and recalculate API endpoints
- *(dashboard)* add handler and register GET /dashboard route
- *(dashboard)* add compliance query
- *(dashboard)* add KPI aggregation and sparkline queries
- *(dashboard)* add types, response struct, and period range helper
- register session detail API route
- add session detail transcript parser with per-call breakdown
- add model_pricing table with seed data

### Changed

- migrate all queries from old sessions table to sessions_v2
- remove old traces.rs and legacy endpoints entirely
- pricing module to support DB-backed rates with fallback

### Fixed

- *(api)* restore legacy POST /traces endpoint for backward compatibility with old CLI
- *(api)* add committed_at to GROUP BY for linked commits query
- *(ui)* fix navigation responsiveness, transcript rendering, file change display, linked commits dedup, and branch tracking from commit-push
- *(stream)* process piggybacked transcript lines on all event types, not just Transcript
- *(dashboard)* cast SUM() results to int8 for sqlx type compatibility
- *(dashboard)* fill sparkline date gaps and parallelize queries
- never drop transcript records when message field is missing
- remove audit log from login to avoid nil org_id FK violation
- fix display github hashes for real commits
- fix warnings
- fix cargo clippy

## [0.2.0](https://github.com/softwaremill/tracevault/compare/v0.1.0...v0.2.0) - 2026-03-23

### Fixed

- fix warnings
- fix cargo clippy
