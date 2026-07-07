#!/usr/bin/env bash
set -euo pipefail

REPO="${MVV_REPO:-viggomeesters/mega-vault-viewer}"
VERSION="${MVV_VERSION:-latest}"
APP_NAME="Mega Vault Viewer.app"
INSTALL_DIR="${MVV_INSTALL_DIR:-/Applications}"
API_ROOT="https://api.github.com/repos/${REPO}"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 2
  fi
}

need curl
need python3
need ditto

arch_name="$(uname -m)"
case "$arch_name" in
  arm64|aarch64) arch_pattern="aarch64|arm64" ;;
  x86_64|amd64) arch_pattern="x86_64|x64|amd64" ;;
  *) echo "Unsupported macOS architecture: $arch_name" >&2; exit 2 ;;
esac

if [[ "$VERSION" == "latest" ]]; then
  release_url="${API_ROOT}/releases/latest"
else
  release_url="${API_ROOT}/releases/tags/${VERSION}"
fi

echo "Fetching Mega Vault Viewer release metadata: ${REPO} ${VERSION}"
release_json="$TMP_DIR/release.json"
if ! curl -fsSL "$release_url" -o "$release_json"; then
  echo "No GitHub release found for ${REPO} (${VERSION})." >&2
  echo "Create a release tag first, for example: git tag -a v0.1.1 -m 'Mega Vault Viewer v0.1.1' && git push origin v0.1.1" >&2
  exit 1
fi

asset_url="$(python3 - "$release_json" "$arch_pattern" <<'PY'
import json, re, sys
path, pattern = sys.argv[1], sys.argv[2]
release = json.load(open(path, encoding='utf-8'))
assets = release.get('assets', [])
regex = re.compile(pattern, re.I)
preferred = []
for asset in assets:
    name = asset.get('name', '')
    if not regex.search(name):
        continue
    if name.endswith(('.dmg', '.zip', '.tar.gz', '.tgz')) and 'mac' in name.lower():
        preferred.append(asset)
for suffix in ('.dmg', '.zip', '.tar.gz', '.tgz'):
    for asset in preferred:
        if asset.get('name', '').endswith(suffix):
            print(asset['browser_download_url'])
            sys.exit(0)
safe_assets = ', '.join(asset.get('name', '') for asset in assets) or '(no assets)'
raise SystemExit(f'No matching macOS asset for arch pattern {pattern}. Available assets: {safe_assets}')
PY
)"

asset_name="${asset_url##*/}"
asset_path="$TMP_DIR/$asset_name"
echo "Downloading $asset_name"
curl -fL "$asset_url" -o "$asset_path"

mount_dir=""
app_source=""
case "$asset_name" in
  *.dmg)
    mount_dir="$TMP_DIR/mount"
    mkdir -p "$mount_dir"
    echo "Mounting DMG"
    hdiutil attach "$asset_path" -mountpoint "$mount_dir" -nobrowse -quiet
    trap '[[ -n "${mount_dir:-}" ]] && hdiutil detach "$mount_dir" -quiet || true; cleanup' EXIT
    app_source="$mount_dir/$APP_NAME"
    ;;
  *.zip)
    unzip_dir="$TMP_DIR/unzip"
    mkdir -p "$unzip_dir"
    ditto -x -k "$asset_path" "$unzip_dir"
    app_source="$(find "$unzip_dir" -maxdepth 3 -name "$APP_NAME" -type d | head -n 1)"
    ;;
  *.tar.gz|*.tgz)
    tar_dir="$TMP_DIR/tar"
    mkdir -p "$tar_dir"
    tar -xzf "$asset_path" -C "$tar_dir"
    app_source="$(find "$tar_dir" -maxdepth 3 -name "$APP_NAME" -type d | head -n 1)"
    ;;
  *)
    echo "Unsupported asset type: $asset_name" >&2
    exit 2
    ;;
esac

if [[ -z "$app_source" || ! -d "$app_source" ]]; then
  echo "Could not find ${APP_NAME} inside $asset_name" >&2
  exit 1
fi

target="$INSTALL_DIR/$APP_NAME"
echo "Installing to $target"
if [[ -d "$target" ]]; then
  rm -rf "$target"
fi
if [[ ! -w "$INSTALL_DIR" ]]; then
  sudo ditto "$app_source" "$target"
  sudo xattr -dr com.apple.quarantine "$target" 2>/dev/null || true
else
  ditto "$app_source" "$target"
  xattr -dr com.apple.quarantine "$target" 2>/dev/null || true
fi

echo "Installed Mega Vault Viewer: $target"
echo "Open it with: open '$target'"
