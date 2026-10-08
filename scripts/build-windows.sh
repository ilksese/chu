#!/usr/bin/env bash
set -euo pipefail
exec "$(dirname "$0")/build.sh" x86_64-pc-windows-msvc
