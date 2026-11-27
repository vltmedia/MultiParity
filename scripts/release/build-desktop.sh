#!/usr/bin/env bash
# Build the desktop app on a Mac (or Linux): signed for updates and, on a Mac with an Apple
# Developer ID set up, code signed and notarized so Gatekeeper opens it without a warning.
#
#   scripts/release/build-desktop.sh [--out <folder>] [--notes "What changed"] [--universal]
#
# macOS code signing (docs/SIGNING.md), read by Tauri from the environment:
#   APPLE_SIGNING_IDENTITY   "Developer ID Application: Your Studio (TEAMID)", in the keychain
#   notarization, either an App Store Connect API key:
#     APPLE_API_ISSUER, APPLE_API_KEY, APPLE_API_KEY_PATH
#   or an Apple ID with an app-specific password:
#     APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID
# Not set: the app is built unsigned (people right-click > Open the first time).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT=""; NOTES=""; TARGET=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --notes) NOTES="$2"; shift 2 ;;
    --universal) TARGET=(--target universal-apple-darwin); shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done

export TAURI_SIGNING_PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY:-$HOME/.tauri/multiparity-updater.key}"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
if [ ! -e "$TAURI_SIGNING_PRIVATE_KEY" ] && [ "${TAURI_SIGNING_PRIVATE_KEY:0:20}" != "dW50cnVzdGVkIGNvbW1l" ]; then
  echo "No update signing key at $TAURI_SIGNING_PRIVATE_KEY. Copy it from the machine that made it (docs/DESKTOP.md)." >&2
  exit 1
fi

if [ "$(uname)" = Darwin ]; then
  if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
    security find-identity -v -p codesigning | grep -q "$APPLE_SIGNING_IDENTITY" || { echo "The keychain has no signing identity \"$APPLE_SIGNING_IDENTITY\"." >&2; exit 1; }
    if [ -n "${APPLE_API_KEY:-}" ] || [ -n "${APPLE_ID:-}" ]; then
      echo "Code signing as $APPLE_SIGNING_IDENTITY, and notarizing."
    else
      echo "warning: signing but not notarizing (set APPLE_API_* or APPLE_ID/APPLE_PASSWORD/APPLE_TEAM_ID); Gatekeeper still warns." >&2
    fi
  else
    echo "warning: no APPLE_SIGNING_IDENTITY: the app is unsigned and Gatekeeper will warn people (docs/SIGNING.md)." >&2
  fi
fi

cd "$ROOT/desktop"
[ -d node_modules ] || pnpm install
npx tauri build "${TARGET[@]}"

if [ "$(uname)" = Darwin ]; then
  APP=$(ls -d "$ROOT"/desktop/src-tauri/target/*/release/bundle/macos/*.app "$ROOT"/desktop/src-tauri/target/release/bundle/macos/*.app 2>/dev/null | head -1)
  [ -n "$APP" ] && { codesign -dv "$APP" 2>&1 | grep -E "Authority|Signature" | head -2 || echo "$(basename "$APP"): not code signed"; }
fi

if [ -n "$OUT" ]; then
  BUNDLE="$ROOT/desktop/src-tauri/target/release/bundle"
  ARCH=""
  if [ ${#TARGET[@]} -gt 0 ]; then BUNDLE="$ROOT/desktop/src-tauri/target/universal-apple-darwin/release/bundle"; ARCH="--arch universal"; fi
  node "$ROOT/scripts/release/desktop-latest.mjs" --out "$OUT" --bundle "$BUNDLE" ${NOTES:+--notes "$NOTES"} $ARCH
fi
