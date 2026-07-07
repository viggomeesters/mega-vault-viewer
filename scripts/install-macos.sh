#!/usr/bin/env bash
set -euo pipefail

REPO="${MVV_REPO:-viggomeesters/mega-vault-viewer}"
VERSION="${MVV_VERSION:-latest}"
APP_NAME="Mega Vault Viewer.app"
INSTALL_DIR="${MVV_INSTALL_DIR:-/Applications}"
SOURCE_DIR="${MVV_SOURCE_DIR:-$HOME/.local/share/mega-vault-viewer/source}"
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

install_app() {
  local app_source="$1"
  if [[ -z "$app_source" || ! -d "$app_source" ]]; then
    echo "Could not find ${APP_NAME}" >&2
    exit 1
  fi

  local target="$INSTALL_DIR/$APP_NAME"
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
}

install_from_source() {
  echo "No usable GitHub release asset found; falling back to source build."
  need git
  need npm
  need cargo
  need ditto

  mkdir -p "$(dirname "$SOURCE_DIR")"
  if [[ ! -d "$SOURCE_DIR/.git" ]]; then
    echo "Cloning ${REPO} to ${SOURCE_DIR}"
    git clone "https://github.com/${REPO}.git" "$SOURCE_DIR"
  fi

  git -C "$SOURCE_DIR" fetch --tags origin
  if [[ "$VERSION" == "latest" ]]; then
    git -C "$SOURCE_DIR" checkout main
    git -C "$SOURCE_DIR" pull --ff-only origin main
  else
    git -C "$SOURCE_DIR" checkout "$VERSION"
  fi

  echo "Building macOS app locally"
  (cd "$SOURCE_DIR" && npm ci && npm run desktop:build:macos)
  local app_source
  app_source="$(find "$SOURCE_DIR/target/release/bundle/macos" -maxdepth 1 -name "$APP_NAME" -type d | head -n 1)"
  install_app "$app_source"
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

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This installer is for macOS. Detected: $(uname -s)" >&2
  exit 2
fi

if [[ "$VERSION" == "latest" ]]; then
  release_url="${API_ROOT}/releases/latest"
else
  release_url="${API_ROOT}/releases/tags/${VERSION}"
fi

echo "Fetching Mega Vault Viewer release metadata: ${REPO} ${VERSION}"
release_json="$TMP_DIR/release.json"
if ! curl -fsSL "$release_url" -o "$release_json"; then
  install_from_source
  exit 0
fi

asset_url=""
selector_output="$TMP_DIR/selector.out"
if python3 - "$release_json" "$arch_pattern" > "$selector_output" <<'PY'
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
then
  asset_url="$(cat "$selector_output")"
else
  cat "$selector_output" >&2 || true
  install_from_source
  exit 0
fi

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
    install_from_source
    exit 0
    ;;
esac

install_app "$app_source"
