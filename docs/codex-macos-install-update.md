# Codex instruction: install/update Mega Vault Viewer on macOS

You are operating on Viggo's MacBook. Install or update **Mega Vault Viewer** from its canonical GitHub repository.

## Goal

Make sure the latest Mega Vault Viewer app is installed at:

```text
/Applications/Mega Vault Viewer.app
```

Use the repo-provided installer/updater. Do not invent a separate installation flow unless this script fails and you have captured the exact error.

## Command

Run this in a macOS terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/viggomeesters/mega-vault-viewer/main/scripts/install-macos.sh | bash
```

For a pinned version, run:

```bash
curl -fsSL https://raw.githubusercontent.com/viggomeesters/mega-vault-viewer/main/scripts/install-macos.sh | MVV_VERSION=v0.1.1 bash
```

## What the script does

1. Detects Mac architecture: `arm64`/Apple Silicon or `x86_64`/Intel.
2. Tries to download the matching GitHub Release asset when available.
3. If no usable release asset exists yet, falls back to a local source build:
   - clones/updates `https://github.com/viggomeesters/mega-vault-viewer.git` under `~/.local/share/mega-vault-viewer/source`
   - runs `npm ci`
   - runs `npm run desktop:build:macos`
   - copies the built app to `/Applications/Mega Vault Viewer.app`
4. Removes macOS quarantine metadata where possible.

## Expected prerequisites for source-build fallback

The release-asset path only needs standard macOS tools. The source-build fallback needs:

```text
git
node/npm
Rust/cargo
Tauri macOS build dependencies
```

If one is missing, install the normal developer dependency and rerun the same command. Do not change the repository code just to work around a missing host dependency.

## Verification

After the script finishes, verify:

```bash
test -d "/Applications/Mega Vault Viewer.app" && echo "installed"
open "/Applications/Mega Vault Viewer.app"
```

If macOS blocks the unsigned app on first launch, report that clearly. The current release channel is unsigned/not notarized unless a specific release says otherwise.

## Failure handling

If the install/update fails:

1. Preserve the exact terminal output.
2. Check whether the failure is:
   - missing host dependency;
   - GitHub/network access;
   - Rust/npm build failure;
   - macOS unsigned/quarantine prompt.
3. Do not claim success unless `/Applications/Mega Vault Viewer.app` exists and the app was opened or the exact launch blocker was reported.

## Canonical references

- Repository: https://github.com/viggomeesters/mega-vault-viewer
- Installer script: https://raw.githubusercontent.com/viggomeesters/mega-vault-viewer/main/scripts/install-macos.sh
- Release guide: https://github.com/viggomeesters/mega-vault-viewer/blob/main/docs/release.md
