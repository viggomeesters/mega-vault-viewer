# Contributing

Thanks for considering a contribution to Mega Vault Viewer.

## Product Principles

- Keep the filesystem vault canonical.
- Treat SQLite, Tantivy, thumbnails, and render caches as rebuildable runtime state.
- Prefer read-first behavior and explicit writes.
- Use fixture or synthetic data in tests, docs, screenshots, and issues.
- Avoid private vault paths, client names, personal notes, and private screenshots.

## Development Setup

```bash
npm install
```

Run the desktop app:

```bash
npm run desktop:dev
```

Run the main checks:

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
npm test --if-present
npm run build --if-present
npm run desktop:build
git diff --check
```

## Pull Request Expectations

- Keep changes scoped to one behavior or repo-readiness improvement.
- Include tests for parser, index, search, or UI behavior when practical.
- Update docs when user-facing behavior changes.
- Do not commit generated runtime indexes, local app state, private screenshots, or build artifacts.
- Use fixtures under `fixtures/` for examples and screenshots.

## Issue Reports

Good issue reports include:

- What you expected.
- What happened.
- Steps to reproduce.
- Platform and commit/version.
- Whether the issue involves reading, indexing, rendering, search, or writing.

Do not attach private vault content. Create a minimal synthetic reproduction instead.
