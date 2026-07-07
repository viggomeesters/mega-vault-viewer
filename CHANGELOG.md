# Changelog

All notable changes to Mega Vault Viewer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project uses explicit tags/releases once public distribution begins.

## Unreleased

### Fixed

- Kept sync fast for JSONL vaults with large CAS stores by skipping `blobs/`, `.obsidian`, and runtime/cache directories during normal index discovery. CAS media still renders lazily from `blobs/sha256/<first2>/<hash>` when a note references it.

## [0.1.1] - 2026-07-07

### Added

- JSONL-vault layout support for `daily/YYYY-MM-DD.md`, top-level folder navigation, CAS blob rendering, remembered vaults, vault-size display, and cleaner independent-scroll reader shell.
- GitHub Releases based macOS install/update channel with Apple Silicon and Intel artifacts.
- `scripts/install-macos.sh` for one-command install/update to `/Applications`.
- Linux AppImage packaging fallback for WSL/CI environments where linuxdeploy/AppImage FUSE handling or icon basename quirks break Tauri bundling.

### Notes

- macOS artifacts are unsigned and not notarized until signing credentials are added.
- The macOS install/update script is a pragmatic release-channel updater, not an in-app auto-updater.

## [0.1.0] - 2026-06-22

### Added

- Public repository documentation baseline.
- Security policy, contribution guide, issue template, and pull request template.
- GitHub Actions CI workflow for Rust, TypeScript, Vite, and Tauri build checks.
- Release guide for local app builds, versioning, tags, and unsigned artifact caveats.
- Public-safe hero, social preview, and representative screenshot assets.
- Visual asset refresh workflow using fixture or synthetic data only.
- Public readiness scrub notes and hardened ignore rules for runtime state, local environment files, and app bundles.
- GitHub publishing metadata and v0.1.0 release notes.

### Notes

- Mega Vault Viewer is currently an MVP local-first macOS desktop app.
- Public release artifacts are not yet guaranteed to be signed or notarized.
