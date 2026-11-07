#!/usr/bin/env bash
# Check Mac host tools needed for firmware Tier 0 and the Tauri BLE bridge.
set -euo pipefail

fail=0

ok() { echo "ok  $1"; }
missing() { echo "MISSING  $1"; fail=1; }

echo "=== Mac desk-pet environment ==="

if command -v rustc >/dev/null 2>&1; then
  ok "rustc $(rustc --version | awk '{print $2}')"
else
  missing "rustc (mise install rust)"
fi

if command -v cargo >/dev/null 2>&1; then
  ok "cargo $(cargo --version | awk '{print $2}')"
else
  missing "cargo"
fi

if command -v cargo-tauri >/dev/null 2>&1 || cargo tauri --version >/dev/null 2>&1; then
  ok "tauri-cli $(cargo tauri --version 2>/dev/null | awk '{print $2}')"
else
  missing "tauri-cli (cargo install tauri-cli --locked)"
fi

if command -v node >/dev/null 2>&1; then
  ok "node $(node --version)"
else
  missing "node"
fi

if command -v pnpm >/dev/null 2>&1; then
  ok "pnpm $(pnpm --version)"
else
  missing "pnpm"
fi

if command -v python3 >/dev/null 2>&1; then
  ok "python $(python3 --version 2>&1 | awk '{print $2}')"
else
  missing "python3"
fi

PIO="${HOME}/.platformio/penv/bin/pio"
if [[ -x "$PIO" ]]; then
  ok "pio $($PIO --version 2>/dev/null | awk '{print $NF}')"
else
  missing "PlatformIO CLI at ~/.platformio/penv/bin/pio"
fi

if [[ -d "${HOME}/.platformio/platforms/espressif32" ]]; then
  ok "platform espressif32"
else
  missing "platform espressif32 (pio run -e esp32-c3-super-mini)"
fi

if [[ -d "${HOME}/.platformio/platforms/native" ]]; then
  ok "platform native"
else
  missing "platform native (pio platform install native)"
fi

if command -v jq >/dev/null 2>&1; then
  ok "jq $(jq --version)"
else
  missing "jq (needed by scripts/cursor-hook.sh and scripts/agent-hook.sh)"
fi

if xcode-select -p >/dev/null 2>&1; then
  ok "xcode-select $(xcode-select -p)"
else
  missing "Xcode Command Line Tools"
fi

echo "=== USB serial ==="
shopt -s nullglob
ports=(/dev/cu.usb* /dev/cu.wchusbserial* /dev/cu.SLAB_USBtoUART*)
if ((${#ports[@]})); then
  for p in "${ports[@]}"; do
    ok "serial $p"
  done
else
  echo "none  no ESP32 USB serial (plug in the Super Mini for flash/monitor)"
fi

if [[ "$fail" -ne 0 ]]; then
  echo "=== FAIL: restore missing tools before Mac bridge work ==="
  exit 1
fi

echo "=== PASS ==="
