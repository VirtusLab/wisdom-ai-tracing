# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
