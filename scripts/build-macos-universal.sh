#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/build.sh" aarch64-apple-darwin
"$ROOT/scripts/build.sh" x86_64-apple-darwin
OUT="${CARGO_TARGET_DIR:-$ROOT/target}"
lipo -create "$OUT/aarch64-apple-darwin/release/chu" "$OUT/x86_64-apple-darwin/release/chu" -output "$ROOT/build/bin/chu"
cp "$ROOT/build/bin/chu" "$ROOT/build/bin/Chu.app/Contents/MacOS/chu"
printf 'Built universal build/bin/Chu.app\n'
