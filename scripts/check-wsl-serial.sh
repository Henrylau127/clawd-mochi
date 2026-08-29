#!/usr/bin/env bash
# Check WSL serial access for Tier 1/2 validation (upload + monitor).
set -euo pipefail

PIO="${HOME}/.platformio/penv/bin/pio"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== Desk Pet WSL serial check ==="

if [[ ! -x "$PIO" ]]; then
  echo "FAIL: pio not found at $PIO"
  exit 1
fi

if groups | tr ' ' '\n' | grep -qx dialout; then
  echo "OK: user is in dialout group"
else
  echo "BLOCKED: user not in dialout group"
  echo "  Fix: sudo usermod -aG dialout \$USER"
  echo "  Then: newgrp dialout   (or log out and back in)"
  exit 2
fi

PORT=""
for candidate in /dev/ttyACM0 /dev/ttyACM1; do
  if [[ -e "$candidate" ]]; then
    PORT="$candidate"
    break
  fi
done

if [[ -z "$PORT" ]]; then
  echo "BLOCKED: no /dev/ttyACM* device (usbipd attach on Windows?)"
  exit 3
fi

if [[ ! -w "$PORT" ]]; then
  echo "BLOCKED: $PORT not writable (dialout membership may need new shell)"
  exit 4
fi

echo "OK: serial port $PORT is writable"

cd "$REPO_ROOT"
echo ""
echo "Tier 0 (compile + tests):"
"$PIO" run -e esp32-c3-super-mini
"$PIO" test -e native -f test_protocol
echo "  Tier 0 PASS"

echo ""
echo "Tier 1 upload:"
echo "  $PIO run -t upload -e esp32-c3-super-mini --upload-port $PORT"
echo ""
echo "Tier 2 monitor (type STATUS, TEST coding, HELP):"
echo "  $PIO device monitor -b 115200 --port $PORT"
