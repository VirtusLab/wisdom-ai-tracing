# Contributing to Visdom Trace

Thank you for your interest in contributing to Visdom Trace! This document explains how to contribute.

## Before You Start

For non-trivial changes, please open an issue first to discuss your proposal. This helps avoid duplicated effort and ensures your contribution aligns with the project's direction.

## AI-Assisted Contributions

We welcome AI-assisted contributions. Whether you write code by hand, use an AI coding assistant, or a combination of both — all contributions are held to the same standard: **every change must include thorough tests.** PRs without adequate test coverage will not be merged, regardless of how the code was written.

## Contributor License Agreement (CLA)

External contributors must sign our [CLA](CLA.md) before their first PR can be merged. An automated bot will prompt you when you open a PR. Members of the `softwaremill` and `virtuslab` GitHub organizations are exempt.

## Development Workflow

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Ensure all checks pass (see below)
5. Open a pull request against `main`

## Code Quality

All PRs must pass these checks before merging:

```bash
# Format check
cargo fmt -- --check

# Lint
cargo clippy --all-targets -- -D warnings

# Tests
cargo test
```

For frontend changes in the `web/` directory:

```bash
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
pnpm test
```

## Commit Messages

Follow the conventional commits style used in this project:

- `feat:` — new feature
- `fix:` — bug fix
- `docs:` — documentation only
- `chore:` — maintenance tasks
- `refactor:` — code change that neither fixes a bug nor adds a feature

## Enterprise Features

Visdom Trace has an enterprise edition. Contributions to the open-source project are welcome, but please note that the enterprise module (`enterprise/`) is not part of the open-source distribution.
