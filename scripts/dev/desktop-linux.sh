#!/usr/bin/env bash
# Build the desktop app for Linux inside a Debian container (used to check it compiles on the
# Linux build host; Windows and macOS builds run on those systems with `pnpm tauri build`).
#   scripts/dev/baiji.sh sh bash scripts/dev/desktop-linux.sh [--bundle]
set -euo pipefail
cd "$(dirname "$0")/../.."
export DEBIAN_FRONTEND=noninteractive
apt-get -qq update >/dev/null
apt-get -qq install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev file >/dev/null
if [ "${1:-}" = "--bundle" ]; then
  apt-get -qq install -y nodejs npm >/dev/null
  cd desktop && npx --yes @tauri-apps/cli@2.5.0 build --bundles deb,appimage
else
  cargo build --manifest-path desktop/src-tauri/Cargo.toml --target-dir target/desktop
fi
