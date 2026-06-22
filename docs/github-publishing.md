# GitHub Publishing

This document records the intended public GitHub metadata and release policy for Mega Vault Viewer.

## Repository

- Owner/name: `viggomeesters/mega-vault-viewer`
- Visibility: public
- Default branch: `main`
- URL: `https://github.com/viggomeesters/mega-vault-viewer`
- Description: `Local-first macOS viewer for large mixed-format Markdown knowledge vaults.`
- Homepage: none for now. Mega Vault Viewer is a desktop app and does not yet have a hosted product site.
- Topics:
  - `local-first`
  - `tauri`
  - `rust`
  - `markdown`
  - `knowledge-base`
  - `search`
  - `macos`

## Publish Commands

Create the public repository and push the current repository state:

```bash
gh repo create viggomeesters/mega-vault-viewer \
  --public \
  --description "Local-first macOS viewer for large mixed-format Markdown knowledge vaults." \
  --source=. \
  --remote=origin

git branch -f main HEAD
git push -u origin main
git push origin codex/mega-vault-viewer-mvp
gh repo edit viggomeesters/mega-vault-viewer \
  --add-topic local-first \
  --add-topic tauri \
  --add-topic rust \
  --add-topic markdown \
  --add-topic knowledge-base \
  --add-topic search \
  --add-topic macos
```

## Release Command

Create an unsigned macOS app zip from a verified local build:

```bash
npm run desktop:build
ditto -c -k --keepParent "target/release/bundle/macos/Mega Vault Viewer.app" /tmp/Mega-Vault-Viewer-v0.1.0-macos-unsigned.zip
gh release create v0.1.0 /tmp/Mega-Vault-Viewer-v0.1.0-macos-unsigned.zip \
  --repo viggomeesters/mega-vault-viewer \
  --title "Mega Vault Viewer v0.1.0" \
  --notes-file docs/release-notes/v0.1.0.md
```

The release artifact is intentionally labeled `unsigned`. Do not imply signing, notarization, auto-update, or installer support until those features exist.

## Published Release

- Tag: `v0.1.0`
- URL: `https://github.com/viggomeesters/mega-vault-viewer/releases/tag/v0.1.0`
- Asset: `Mega-Vault-Viewer-v0.1.0-macos-unsigned.zip`
