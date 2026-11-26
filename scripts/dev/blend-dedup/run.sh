#!/usr/bin/env bash
# Build a large .blend, save it again after typical edits (one Blender session per stage), and
# leave the versions in <dir> for the dedup example to measure.
#
#   scripts/dev/blend-dedup/run.sh <dir> [target]     target 8.0 gives files of about 4 GB
#
# Needs Blender on PATH. Then, from the repository root:
#   cargo run --release -p mp-core --example dedup -- <dir>/v1.blend <dir>/v2.blend <dir>/n1.blend \
#     <dir>/v3.blend <dir>/v4.blend <dir>/v5.blend <dir>/v6.blend <dir>/v7.blend
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
DIR="$(mkdir -p "$1" && cd "$1" && pwd)"
TARGET="${2:-8.0}"
rm -f "$DIR"/*.blend "$DIR"/*.blend1
for stage in create noop move sculpt add zip zipmove; do
  blender -b --python "$HERE/make_blends.py" -- "$stage" "$DIR" "$TARGET" 2>&1 | grep -E "STAGE|built|SAVED|Error|Traceback|rror:" || true
done
ls -la "$DIR"
echo DONE
