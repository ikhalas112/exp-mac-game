#!/usr/bin/env bash
# Local macOS release pipeline test for the real Snake desktop game.
#   resolve -> dx bundle (build .app) -> stage into build/ -> inject-version
#   -> package (ditto -c -k) -> manifest
# sync / update-channel / verify are skipped (need R2 creds + live CDN).
# Usage: run-release-local.sh [tag] [channel]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENGINE="${ENGINE_BIN:-$ROOT/../maxgame-release-tools/target/debug/maxgame-release}"
CONFIG="$ROOT/release.config.json"
TAG="${1:-v0.1.0-dev}"
CHANNEL="${2:-dev}"

APP_NAME="SnakeDesktop.app"
DX_OUT="$ROOT/target/dx/snake-desktop/bundle/macos/macos"
OUTPUT_ZIP="output/SnakeDesktop-macos.zip"

echo "=== mock-mac-game local release test ==="
echo "root:     $ROOT"
echo "engine:   $ENGINE"
echo "tag:      $TAG"
echo "platform: macos"
echo

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macos lane requires a macOS host (ditto + dx bundle)" >&2
  exit 1
fi

if [[ ! -x "$ENGINE" ]]; then
  echo "Building maxgame-release..."
  (cd "$ROOT/../maxgame-release-tools" && cargo build -q)
fi

cd "$ROOT"

# 1. resolve tag -> env/channel/macos R2 paths
echo "--- resolve (macos) ---"
"$ENGINE" resolve "$TAG" --config="$CONFIG" --format=env --platform=macos | head -8
echo

# 2. build the real .app via the Dioxus CLI (reuse if already built)
if [[ ! -d "$DX_OUT/$APP_NAME" ]]; then
  echo "--- dx bundle (building $APP_NAME) ---"
  command -v dx >/dev/null || { echo "dx not found; run: cargo install dioxus-cli" >&2; exit 1; }
  dx bundle --desktop --package snake-desktop
else
  echo "--- dx bundle (reusing existing $APP_NAME) ---"
fi
echo

# 3. stage the built .app into build/ (source: build)
echo "--- stage .app -> build/ ---"
rm -rf build
mkdir -p build
ditto "$DX_OUT/$APP_NAME" "build/$APP_NAME"
echo "staged build/$APP_NAME"
echo

# 4. inject version.txt into the bundle's Resources (before packaging)
echo "--- inject-version ---"
"$ENGINE" inject-version --source="build/$APP_NAME/Contents/Resources" --tag="$TAG" --channel="$CHANNEL"
echo

# 5. package the .app into a single zip via ditto
echo "--- package (ditto) ---"
"$ENGINE" package --config="$CONFIG" --build-dir=build --output-dir=output
echo

# 6. generate manifest.json + channel-manifest.json
echo "--- manifest (macos) ---"
"$ENGINE" manifest --config="$CONFIG" --tag="$TAG" --artifact="$OUTPUT_ZIP" --output-dir=output --platform=macos
echo

echo "=== done (macos) ==="
echo "output:"
ls -la output/
echo
echo "manifest.json:"
cat output/manifest.json
