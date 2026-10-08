#!/usr/bin/env bash
set -euo pipefail
# Cross compilation requires each target's Rust toolchain, linker and platform SDK.
# Run targets on their native OS in CI instead of pretending a failed cross-build succeeded.
if [[ $# -eq 0 ]]; then
  printf 'Usage: %s <rust-target> [rust-target ...]\nExample: %s aarch64-apple-darwin x86_64-apple-darwin\n' "$0" "$0"
  exit 2
fi
for target in "$@"; do "$(dirname "$0")/build.sh" "$target"; done
