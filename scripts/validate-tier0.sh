#!/usr/bin/env bash
# Tier 0: build + host unit tests (no hardware).
set -euo pipefail

PIO="${HOME}/.platformio/penv/bin/pio"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cd "$REPO_ROOT"

echo "=== Tier 0 validation ==="
"$PIO" run -e esp32-c3-super-mini
"$PIO" run -e esp32-c3-super-mini-debug
"$PIO" test -e native -f test_protocol
echo "Tier 0 PASS"
