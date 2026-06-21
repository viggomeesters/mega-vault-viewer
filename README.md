# Mega Vault Viewer

Mega Vault Viewer is a local-first, read-first macOS vault viewer for large mixed-format knowledge vaults. The vault filesystem is canonical: Markdown notes, source files, media and future file types stay in their original formats. SQLite, Tantivy and render artifacts are rebuildable shadow indexes owned by the app.

## Current MVP

- Rust core crate: `crates/mvv-core`
- Desktop shell: `apps/desktop` using Tauri v2
- Fixture vault: `fixtures/demo-vault`
- SQLite shadow cache: document metadata, paths, links and backlinks
- Tantivy shadow cache: title/body/slug full-text search
- UI flow: choose vault path, index, search, open notes, follow WikiLinks

## Runtime State

Normal desktop runs store runtime state under the platform app data directory, for example `~/Library/Application Support/Mega Vault Viewer/` on macOS. The app must not create SQLite, WAL, SHM, Tantivy, thumbnail or render-cache artifacts in the vault root during startup or indexing.

Set `MEGA_VAULT_VIEWER_STATE_DIR=/path/to/state` to run against an explicit rebuildable state directory for development, tests or fixtures. Resetting the index removes only known runtime artifacts from that state directory; vault files remain the source of truth.

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
