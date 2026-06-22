# Release Guide

Mega Vault Viewer releases are explicit and evidence-backed. The current project state supports local macOS app bundle builds. It does not yet provide signing, notarization, auto-update, or a packaged installer unless a specific release says otherwise.

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
npm run desktop:build
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

## Build Artifact

Create the local macOS app bundle on macOS:

```bash
npm run desktop:build
```

The app bundle is generated at:

```text
target/release/bundle/macos/Mega Vault Viewer.app
```

Do not commit files under `target/`.

WSL2 is suitable for source development and verification, but release packaging remains macOS-only until the project explicitly supports Linux or Windows desktop artifacts.

## Tagging

Use an annotated tag:

```bash
git tag -a v0.1.0 -m "Mega Vault Viewer v0.1.0"
git push origin v0.1.0
```

## GitHub Release

Create a GitHub release only after the verification suite passes on a clean checkout.

Release notes should state:

- Supported platform.
- Whether the app is signed or unsigned.
- Whether notarization is available.
- Which artifact is attached.
- Known limitations for local-first vault access and runtime indexes.

Until signing and notarization are implemented, release notes must clearly state that any attached macOS bundle is unsigned.

## Not Yet Supported

- Code signing.
- Apple notarization.
- Auto-update.
- Installer packages.
- Cross-platform builds.
