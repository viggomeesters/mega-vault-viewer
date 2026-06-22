# GitHub Publishing

Mega Vault Viewer is published as a public GitHub repository.

## Published Repository

- Owner/name: `viggomeesters/mega-vault-viewer`
- Visibility: public
- Default branch: `main`
- URL: `https://github.com/viggomeesters/mega-vault-viewer`
- Description: `Local-first macOS viewer for large mixed-format Markdown knowledge vaults.`
- Homepage: none. Mega Vault Viewer is a desktop app and does not have a hosted product site.
- Current release: `v0.1.0`
- Release URL: `https://github.com/viggomeesters/mega-vault-viewer/releases/tag/v0.1.0`
- Release asset: `Mega-Vault-Viewer-v0.1.0-macos-unsigned.zip`

## Repository Topics

- `local-first`
- `tauri`
- `rust`
- `markdown`
- `knowledge-base`
- `search`
- `macos`

## Public Visuals

The public README and social assets are fixture-safe:

- README hero: `assets/hero.svg`
- Social preview asset: `assets/social-preview.svg`
- Fixture screenshot: `assets/screenshot-fixture-vault.svg`

See [visual-assets.md](visual-assets.md) for rendered previews.

## Release Policy

The v0.1.0 artifact is intentionally labeled `unsigned`. Do not imply signing, notarization, auto-update, or installer support until those features exist.

Public releases should stay evidence-backed:

1. Run the verification suite from [release.md](release.md).
2. Build the macOS app bundle.
3. Package the unsigned app zip with an explicit `unsigned` filename.
4. Publish GitHub release notes from `docs/release-notes/`.
5. Re-run the public privacy scrub before attaching artifacts.

## Maintenance Commands

These commands are for future metadata or release maintenance, not first-time publication.

Update GitHub topics:

```bash
gh repo edit viggomeesters/mega-vault-viewer \
  --add-topic local-first \
  --add-topic tauri \
  --add-topic rust \
  --add-topic markdown \
  --add-topic knowledge-base \
  --add-topic search \
  --add-topic macos
```

Create a future unsigned macOS release after a verified local build:

```bash
npm run desktop:build
ditto -c -k --keepParent "target/release/bundle/macos/Mega Vault Viewer.app" /tmp/Mega-Vault-Viewer-vNEXT-macos-unsigned.zip
gh release create vNEXT /tmp/Mega-Vault-Viewer-vNEXT-macos-unsigned.zip \
  --repo viggomeesters/mega-vault-viewer \
  --title "Mega Vault Viewer vNEXT" \
  --notes-file docs/release-notes/vNEXT.md
```
