#!/usr/bin/env bash
# Build the desktop app on a Mac (or Linux): signed for updates and, on a Mac with an Apple
# Developer ID set up, code signed and notarized so Gatekeeper opens it without a warning.
#
#   scripts/release/build-desktop.sh [--out <folder>] [--notes "What changed"] [--base-url <url>] [--universal]
#
# macOS code signing (docs/SIGNING.md), read by Tauri from the environment:
#   APPLE_SIGNING_IDENTITY   "Developer ID Application: Your Studio (TEAMID)", in the keychain
#   notarization, either an App Store Connect API key:
#     APPLE_API_ISSUER, APPLE_API_KEY, APPLE_API_KEY_PATH
#   or an Apple ID with an app-specific password:
#     APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID
#   MP_SIGNING_KEYCHAIN      optional: a keychain holding only the signing identity, searched alone
#                            while signing (needed when the same identity is also in the login
#                            keychain); unlocked with the password in MP_SIGNING_KEYCHAIN_PASS_FILE.
#                            Signing over SSH needs this: the login keychain stays locked there.
# Not set: the app is built unsigned (people right-click > Open the first time).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT=""; NOTES=""; BASE=""; TARGET=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --notes) NOTES="$2"; shift 2 ;;
    --base-url) BASE="$2"; shift 2 ;;
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
  if [ -n "${MP_SIGNING_KEYCHAIN:-}" ]; then
    if [ -n "${MP_SIGNING_KEYCHAIN_PASS_FILE:-}" ]; then
      security unlock-keychain -p "$(cat "$MP_SIGNING_KEYCHAIN_PASS_FILE")" "$MP_SIGNING_KEYCHAIN"
    fi
    SAVED_KEYCHAINS="$(security list-keychains -d user | tr -d '"' | tr '\n' ' ')"
    # shellcheck disable=SC2086
    trap 'security list-keychains -d user -s $SAVED_KEYCHAINS' EXIT
    security list-keychains -d user -s "$MP_SIGNING_KEYCHAIN"
  fi
  # Over SSH there is no Finder to arrange the disk image's window; Tauri skips that with CI set.
  [ -n "${SSH_CONNECTION:-}" ] && export CI=true
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
  APP=$( (ls -d "$ROOT"/desktop/src-tauri/target/*/release/bundle/macos/*.app "$ROOT"/desktop/src-tauri/target/release/bundle/macos/*.app 2>/dev/null || true) | head -1)
  if [ -n "$APP" ]; then
    codesign -dvv "$APP" 2>&1 | grep -E "^Authority=Developer" || echo "$(basename "$APP"): not code signed"
    spctl -a "$APP" 2>/dev/null && echo "Gatekeeper accepts it." || echo "Gatekeeper does not accept it yet (not notarized)."
  fi
fi

if [ -n "$OUT" ]; then
  BUNDLE="$ROOT/desktop/src-tauri/target/release/bundle"
  ARCH=""
  if [ ${#TARGET[@]} -gt 0 ]; then BUNDLE="$ROOT/desktop/src-tauri/target/universal-apple-darwin/release/bundle"; ARCH="--arch universal"; fi
  node "$ROOT/scripts/release/desktop-latest.mjs" --out "$OUT" --bundle "$BUNDLE" ${NOTES:+--notes "$NOTES"} ${BASE:+--base-url "$BASE"} $ARCH
fi
