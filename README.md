# Mega Vault Viewer

Mega Vault Viewer is a local-first, read-first macOS vault viewer for large Markdown knowledge bases. The MVP focuses on the first Obsidian replacement slice: render Markdown, resolve WikiLinks, store a graph index in SQLite and search content with Tantivy.

## Current MVP

- Rust core crate: `crates/mvv-core`
- Desktop shell: `apps/desktop` using Tauri v2
- Fixture vault: `fixtures/demo-vault`
- SQLite cache: document metadata, paths, links and backlinks
- Tantivy cache: title/body/slug full-text search
- UI flow: choose vault path, index, search, open notes, follow WikiLinks

## Run

```bash
npm install
npm run desktop:dev
```

The app defaults to `/Users/viggomeesters/Library/Mobile Documents/iCloud~md~obsidian/Documents/vault`.
Replace the path with any local folder containing `.md` files when needed.

## Build macOS App

```bash
npm run desktop:build
```

The generated `.app` bundle is written under `apps/desktop/src-tauri/target/release/bundle/macos/`.

## Verify

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
npm test --if-present
npm run build --if-present
git diff --check
```
