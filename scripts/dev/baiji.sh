#!/usr/bin/env bash
# Development loop on the Linux build host (Docker only, nothing installed on the host).
#
#   scripts/dev/baiji.sh sync              copy the working tree to the host
#   scripts/dev/baiji.sh cargo <args...>   sync, then run cargo in a rust container
#   scripts/dev/baiji.sh sh <command>      sync, then run a shell command in the rust container
#   scripts/dev/baiji.sh host <command>    run a command on the host in the project folder
#
# Environment: MP_DEV_HOST (default UAI-BAIJI), MP_DEV_DIR (default ~/Projects/UAI/MultiParity/multiparity).
set -euo pipefail
HOST="${MP_DEV_HOST:-UAI-BAIJI}"
DIR="${MP_DEV_DIR:-Projects/UAI/MultiParity/multiparity}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
IMAGE="rust:1-bookworm"

sync() {
  tar -C "$ROOT" -czf - \
    --exclude=./target --exclude=./web/node_modules --exclude=./web/dist --exclude=./.git \
    --exclude=./desktop/node_modules --exclude=./desktop/src-tauri/target --exclude=./desktop/src-tauri/gen \
    --exclude='./tmp' . \
  | ssh "$HOST" "mkdir -p $DIR && tar -C $DIR -xzf - 2>/dev/null"
}

in_rust() {
  ssh "$HOST" "cd $DIR && docker run --rm --network host \
    -v \$PWD:/src -w /src \
    -v mp-cargo-registry:/usr/local/cargo/registry \
    -v mp-cargo-git:/usr/local/cargo/git \
    -v mp-target:/src/target \
    -e CARGO_TERM_COLOR=never -e RUST_BACKTRACE=1 \
    -e MP_TEST_DATABASE_URL=postgres://mp:mp@127.0.0.1:18501/mp_test \
    $IMAGE bash -c $(printf '%q' "$*")"
}

cmd="${1:-}"; shift || true
case "$cmd" in
  sync) sync ;;
  cargo) sync; in_rust "cargo $*" ;;
  sh) sync; in_rust "$*" ;;
  host) ssh "$HOST" "cd $DIR && $*" ;;
  *) echo "usage: $0 sync|cargo|sh|host ..." >&2; exit 2 ;;
esac
