# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.16.0](https://github.com/softwaremill/tracevault/compare/v0.15.0...v0.16.0) - 2026-05-27

### Added

- agent-policies — server-rendered policy instructions for agents

## [0.15.0](https://github.com/softwaremill/tracevault/compare/v0.14.0...v0.15.0) - 2026-05-22

### Changed

- *(cli)* remove unused _cwd param from run_stream

### Documentation

- rename product TraceVault → Visdom Trace across all documentation

### Fixed

- *(cli)* resolve project_root from hook_event.cwd instead of process cwd

## [0.14.0](https://github.com/softwaremill/tracevault/compare/v0.13.0...v0.14.0) - 2026-05-22

### Added

- *(policies)* validation window for scoped policy enforcement

### Fixed

- *(policies)* address self-review findings

## [0.13.0](https://github.com/softwaremill/tracevault/compare/v0.12.0...v0.13.0) - 2026-05-21

### Added

- *(init)* add --no-gitignore flag to skip .gitignore updates
- *(policies)* add must_succeed flag to tool call policies

### Changed

- *(init)* remove unused claude_target param from update_root_gitignore

### Fixed

- *(auth)* move device status poll off strict rate limiter and handle 429 in CLI
- *(init)* always gitignore both .claude/settings.json and settings.local.json

## [0.12.0](https://github.com/softwaremill/tracevault/compare/v0.11.3...v0.12.0) - 2026-05-08

### Added

- *(init)* add --claude-settings flag to choose shared vs local hooks

## [0.11.3](https://github.com/softwaremill/tracevault/compare/v0.11.2...v0.11.3) - 2026-04-23

### Fixed

- *(cli)* keep all tracevault files local, update root .gitignore on init

## [0.11.2](https://github.com/softwaremill/tracevault/compare/v0.11.1...v0.11.2) - 2026-04-23

### Fixed

- *(cli)* remove broken fixed-width box from login URL display

## [0.11.1](https://github.com/softwaremill/tracevault/compare/v0.11.0...v0.11.1) - 2026-04-23

### Fixed

- *(cli)* fix flush 413 loop, add timeout/progress, fix status pending check

## [0.11.0](https://github.com/softwaremill/tracevault/compare/v0.10.0...v0.11.0) - 2026-04-23

## [0.10.0](https://github.com/softwaremill/tracevault/compare/v0.9.0...v0.10.0) - 2026-04-22

### Fixed

- *(cli)* make login work in headless environments (Docker, CI, SSH)

## [0.6.2](https://github.com/softwaremill/tracevault/compare/v0.6.1...v0.6.2) - 2026-04-01

### Fixed

- use rustls-tls for CLI and macos-latest for x86_64 builds

## [0.6.1](https://github.com/softwaremill/tracevault/compare/v0.6.0...v0.6.1) - 2026-03-29

### Test

- add CLI unit tests (config, hooks, init, commit_push)

## [0.6.0](https://github.com/softwaremill/tracevault/compare/v0.5.0...v0.6.0) - 2026-03-29

### Added

- add hook adapter architecture with multi-tool detection
- add tool field to streaming protocol v2

## [0.5.0](https://github.com/softwaremill/tracevault/compare/v0.4.0...v0.5.0) - 2026-03-28

### Changed

- remove git-ai, compute attribution server-side from sessions

## [0.4.0](https://github.com/softwaremill/tracevault/compare/v0.3.2...v0.4.0) - 2026-03-25

### Added

- add commit message storage and display

## [0.3.2](https://github.com/softwaremill/tracevault/compare/v0.3.1...v0.3.2) - 2026-03-25

### Added

- send SessionEnd on Claude Code Stop hook

## [0.3.0](https://github.com/softwaremill/tracevault/compare/v0.2.0...v0.3.0) - 2026-03-25

### Added

- *(init)* update hooks for streaming architecture
- *(cli)* add commit-push and flush commands
- *(cli)* add stream command with transcript piggybacking and pending queue
- *(core)* add streaming types, file change extraction, and repo_id to config

## [0.2.0](https://github.com/softwaremill/tracevault/compare/v0.1.0...v0.2.0) - 2026-03-23

### Fixed

- fix tests
- fix cargo clippy
