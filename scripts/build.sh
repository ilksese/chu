#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
TARGET="${1:-}"
CARGO_OUT="${CARGO_TARGET_DIR:-$ROOT/target}"
if [[ -n "$TARGET" ]]; then
  cargo build --release --locked --target "$TARGET"
  SOURCE="$CARGO_OUT/$TARGET/release"
else
  cargo build --release --locked
  SOURCE="$CARGO_OUT/release"
  TARGET="$(rustc -vV | sed -n 's/^host: //p')"
fi
mkdir -p build/bin
if [[ "$TARGET" == *windows* ]]; then
  cp "$SOURCE/chu.exe" build/bin/chu.exe
  printf 'Built build/bin/chu.exe\n'
else
  cp "$SOURCE/chu" build/bin/chu
  printf 'Built build/bin/chu\n'
fi
if [[ "$TARGET" == *apple-darwin* ]]; then
  "$ROOT/scripts/package-macos.sh" "$SOURCE/chu" "$ROOT/build/bin/Chu.app"
fi
