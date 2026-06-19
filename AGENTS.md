# AGENTS.md - Mega Vault Viewer

This repository builds a local-first vault viewer that can eventually replace the Obsidian read path for large Life OS vaults.

## Product Direction

- Read-first before edit-first.
- Markdown is the first source format, not the final runtime model.
- SQLite owns metadata, graph, validation state and fast structured queries.
- Tantivy owns full-text search.
- Source files remain the source of truth; indexes are rebuildable caches.
- Agent write operations must stay transaction-based and opt-in.

## Commands

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
npm test --if-present
npm run build --if-present
npm run desktop:build
```

Use TDD for new behavior. Keep UI changes verified through the actual desktop/web app when possible.
