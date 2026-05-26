# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.16.0](https://github.com/softwaremill/tracevault/compare/v0.15.0...v0.16.0) - 2026-05-26

### Added

- New `agent_policies` module: pure-function Markdown renderer that produces
  agent-readable instructions from the active policies for a repo. Used by
  the CLI's `tracevault agent-policies` command, the `agent_policies` MCP
  tool, and the dashboard preview.

### Changed

- `PolicyAction` now derives `Copy`, `PartialEq`, and `Eq`. Backwards compatible.

## [0.15.0](https://github.com/softwaremill/tracevault/compare/v0.14.0...v0.15.0) - 2026-05-22

### Documentation

- rename product TraceVault → Visdom Trace across all documentation

## [0.14.0](https://github.com/softwaremill/tracevault/compare/v0.13.0...v0.14.0) - 2026-05-22

### Added

- *(policies)* validation window for scoped policy enforcement

### Changed

- *(policies)* address nit comments

## [0.13.0](https://github.com/softwaremill/tracevault/compare/v0.12.0...v0.13.0) - 2026-05-21

### Added

- *(policies)* add must_succeed flag to tool call policies

### Changed

- address review comments

## [0.11.1](https://github.com/softwaremill/tracevault/compare/v0.11.0...v0.11.1) - 2026-04-23

### Fixed

- *(cli)* fix flush 413 loop, add timeout/progress, fix status pending check

## [0.10.0](https://github.com/softwaremill/tracevault/compare/v0.9.0...v0.10.0) - 2026-04-22

## [0.9.0](https://github.com/softwaremill/tracevault/compare/v0.8.0...v0.9.0) - 2026-04-22

### Changed

- *(policy)* honest action set, shared evaluator, exact tool matching, RBAC

## [0.8.0](https://github.com/softwaremill/tracevault/compare/v0.7.0...v0.8.0) - 2026-04-14

### Added

- add SsoProvider trait and community stub

## [0.7.0](https://github.com/softwaremill/tracevault/compare/v0.6.2...v0.7.0) - 2026-04-09

### Added

- add chat_search feature flag, ChatUse permission, and pgvector/fastembed deps

## [0.6.1](https://github.com/softwaremill/tracevault/compare/v0.6.0...v0.6.1) - 2026-03-29

### Test

- add code_nav, hooks, redact, diff, streaming, software tests
- add session state unit tests
- add policy_engine unit tests

## [0.6.0](https://github.com/softwaremill/tracevault/compare/v0.5.0...v0.6.0) - 2026-03-29

### Added

- add tool field to streaming protocol v2

## [0.5.0](https://github.com/softwaremill/tracevault/compare/v0.4.0...v0.5.0) - 2026-03-28

### Added

- add extract_software command parser

### Changed

- remove git-ai, compute attribution server-side from sessions

## [0.4.0](https://github.com/softwaremill/tracevault/compare/v0.3.2...v0.4.0) - 2026-03-25

### Added

- add commit message storage and display

### Fixed

- add missing message field in CommitPushRequest test

## [0.3.1](https://github.com/softwaremill/tracevault/compare/v0.3.0...v0.3.1) - 2026-03-25

## [0.3.0](https://github.com/softwaremill/tracevault/compare/v0.2.0...v0.3.0) - 2026-03-25

### Added

- *(core)* add streaming types, file change extraction, and repo_id to config

## [0.2.0](https://github.com/softwaremill/tracevault/compare/v0.1.0...v0.2.0) - 2026-03-23

### Fixed

- fix cargo clippy
