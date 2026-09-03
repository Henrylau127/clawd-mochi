#!/usr/bin/env bash
# Hardware-free host tests: firmware protocol + simulated Mac bridge.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Firmware native protocol tests ==="
"$HOME/.platformio/penv/bin/pio" test -e native -f test_protocol

echo "=== Desk pet bridge (simulated BLE, no hardware) ==="
cargo test --manifest-path desk-pet-bridge/src-tauri/Cargo.toml --lib

echo "Host tests PASS"
