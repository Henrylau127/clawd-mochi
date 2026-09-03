# Mac validation guide

This document describes how to run the Desk Pet firmware and Tauri menu-bar bridge on macOS. Firmware that already passed WSL Tiers 0–2 is documented in [wsl-validation.md](wsl-validation.md). Session wrap-up (done vs remaining) is in [mac-bridge-status.md](mac-bridge-status.md).

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

## Hardware-free tests / simulated pet

If the ESP32 Super Mini is not plugged in:

```bash
./scripts/validate-host.sh
```

That runs firmware `test_protocol` on the host and the Tauri bridge against an in-process `DP-TEST` mock (no Bluetooth).

Run the menu bar against the mock:

```bash
cd desk-pet-bridge
DESKPET_BLE_MOCK=1 pnpm tauri dev
```

`GET http://127.0.0.1:8765/v1/status` should show `device: DP-TEST` and `ble: connected`. Cursor hooks still POST to loopback; the mock applies firmware-equivalent state.

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

1. Menu-bar icon opens a popover panel (not a dropdown that launches a separate window).
2. macOS Bluetooth permission prompt is accepted.
3. Start the device-side pairing window with serial `PAIR` or a GPIO9 long-press, then use
   Settings → **Add pet**. The bridge shows scanning and passkey-pending states, performs a secure
   GATT handshake, and the passkey appears on the TFT. With no advertising ESP32, Add pet correctly
   enrolls nothing — see [mac-bridge-status.md](mac-bridge-status.md).
4. `curl -s http://127.0.0.1:8765/v1/status` shows `connected` and a `DP-` name only after the
   encrypted status read, mandatory status subscription, and firmware secure-session confirmation.
5. `curl -s -X POST http://127.0.0.1:8765/v1/test -H 'Content-Type: application/json' -d '{"state":"coding"}'` changes the face.
6. Cursor hooks in `.cursor/hooks.json` POST to the same loopback port.

The bridge cannot programmatically remove a CoreBluetooth bond. After **Remove pet**, use System
Settings → Bluetooth → the pet → **Forget This Device** before enrolling it again.

See [desk-pet-bridge/README.md](../desk-pet-bridge/README.md) for API details.
