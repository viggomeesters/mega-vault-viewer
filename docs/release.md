# Release Guide

Mega Vault Viewer releases are explicit and evidence-backed. The current project state supports GitHub Releases based macOS install/update artifacts. It does not yet provide Apple signing, notarization, or in-app auto-update unless a specific release says otherwise.

## Release Readiness Checklist

For cross-platform development checks, including WSL2:

```bash
npm run check
```

Before tagging a macOS release, run the full release gate on macOS:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --all-targets -- -D warnings
npm test --if-present
npm run build --if-present
npm run desktop:build:macos
git diff --check
```

Also run the public repository readiness check when preparing a public GitHub release:

```bash
python3 /path/to/repo_complete_bootstrap.py --public --mode check --path .
```

The repository readiness helper is external to this repository. It is used as a maintainer gate, not as runtime product code.

## Version And Changelog

1. Update the Tauri app version in `apps/desktop/src-tauri/tauri.conf.json`.
2. Update `apps/desktop/package.json` when the desktop package version changes.
3. Update `CHANGELOG.md` with user-visible changes and release caveats.
4. Commit the release preparation changes.

## Build Artifacts

Create the local macOS app bundle and DMG on macOS:

```bash
npm run desktop:build:macos
```

Artifacts are generated under:

```text
target/release/bundle/macos/Mega Vault Viewer.app
target/release/bundle/dmg/*.dmg
```

Do not commit files under `target/`.

WSL2 is suitable for source development and verification. Release packaging is host-specific: macOS `.app` bundles should be built on macOS, and Windows `nsis`/`msi` installers should be built on Windows.

## Tagging And GitHub Release

Use an annotated tag. Pushing a `v*` tag triggers `.github/workflows/release-macos.yml`, which builds both Apple Silicon and Intel macOS assets and attaches them to a GitHub Release:

```bash
git tag -a v0.1.1 -m "Mega Vault Viewer v0.1.1"
git push origin v0.1.1
```

The release workflow uploads:

```text
Mega_Vault_Viewer-macos-aarch64.zip
Mega_Vault_Viewer-macos-aarch64.dmg
Mega_Vault_Viewer-macos-aarch64.sha256
Mega_Vault_Viewer-macos-x86_64.zip
Mega_Vault_Viewer-macos-x86_64.dmg
Mega_Vault_Viewer-macos-x86_64.sha256
```

## Install / Update On macOS

Install or update the latest release:

```bash
curl -fsSL https://raw.githubusercontent.com/viggomeesters/mega-vault-viewer/main/scripts/install-macos.sh | bash
```

Install a specific release:

```bash
MVV_VERSION=v0.1.1 bash scripts/install-macos.sh
```

The script detects `arm64`/`x86_64`, downloads the matching release asset when present, and otherwise falls back to a local source build from GitHub. In both paths it copies `Mega Vault Viewer.app` to `/Applications` and removes quarantine metadata. This is a pragmatic update channel, not an in-app auto-updater.

Until signing and notarization are implemented, release notes must clearly state that macOS artifacts are unsigned.

## Not Yet Supported

- Code signing.
- Apple notarization.
- In-app auto-update.
- Homebrew cask.
