# Mac validation guide

This document describes how to run the Desk Pet firmware and Tauri menu-bar bridge on macOS. Firmware that already passed WSL Tiers 0–2 is documented in [wsl-validation.md](wsl-validation.md).

## Restore host tools

```bash
./scripts/check-mac-env.sh
```

Expected:

- `pio` 6.1.19 at `~/.platformio/penv/bin/pio` (also on PATH after `mise activate` via [mise.toml](../mise.toml))
- native + espressif32 platforms
- Rust / cargo / `tauri-cli`
- Node + pnpm
- jq
- Xcode Command Line Tools

Install missing pieces:

```bash
# PlatformIO native host tests
~/.platformio/penv/bin/pio platform install native

# Tauri CLI
cargo install tauri-cli --locked
```

## Tier 0 on this Mac

```bash
./scripts/validate-tier0.sh
```

## Flash over native USB

Connect the ESP32-C3 Super Mini, then:

```bash
ls /dev/cu.usb*
~/.platformio/penv/bin/pio run -t upload -e esp32-c3-super-mini
~/.platformio/penv/bin/pio device monitor -b 115200
```

If the serial device is missing, unplug/replug and hold BOOT while tapping reset.

## Tier 5 — Tauri bridge

```bash
cd desk-pet-bridge
pnpm install
pnpm tauri dev
```

Checklist:

1. Menu-bar icon appears (no dock icon).
2. macOS Bluetooth permission prompt is accepted.
3. Tray **Start pairing** (or serial `PAIR`) opens the 120s window; passkey is on the TFT.
4. `curl -s http://127.0.0.1:8765/v1/status` shows `connected` and a `DP-` name.
5. `curl -s -X POST http://127.0.0.1:8765/v1/test -H 'Content-Type: application/json' -d '{"state":"coding"}'` changes the face.
6. Cursor hooks in `.cursor/hooks.json` POST to the same loopback port.

See [desk-pet-bridge/README.md](../desk-pet-bridge/README.md) for API details.
