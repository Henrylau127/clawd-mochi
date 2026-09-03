# Desk Pet Bridge (macOS)

Tauri v2 menu-bar app that forwards Cursor agent events to the ESP32 desk pet over encrypted BLE.

## What it does

- Lives in the macOS menu bar (no dock icon).
- Scans for `DP-<MAC>` and the desk-pet GATT service.
- Writes compact JSON to Command RX (`7d3f1b2e-...d002`) using CoreBluetooth via `btleplug`.
- Exposes a loopback HTTP API on `127.0.0.1:8765` for Cursor hooks.
- Queues events while BLE is reconnecting (max 64).
- Never logs passkeys, tokens, or bond material.

## Prerequisites

On the Mac:

```bash
./scripts/check-mac-env.sh
```

Needs Rust, `tauri-cli`, Node/pnpm, Xcode Command Line Tools, and Bluetooth permission when first launched.

## Run

```bash
cd desk-pet-bridge
pnpm install
pnpm tauri dev
```

Tray actions: Reconnect, Start pairing, Stop pairing, Test pet, Show status, Quit.

macOS will prompt for Bluetooth access. Pairing uses the ESP32 120s window and the passkey shown on the TFT.

## Loopback API

Bound only to `127.0.0.1:8765`.

| Method | Path | Body |
|--------|------|------|
| POST | `/v1/cursor/event` | Cursor hook or desk-pet JSON |
| POST | `/v1/test` | `{"state":"coding"}` |
| GET | `/v1/status` | — |
| POST | `/v1/power` | `{"action":"sleep"}` or `{"action":"wake"}` |
| POST | `/v1/pairing/start` | — |
| POST | `/v1/pairing/stop` | — |

Example:

```bash
curl -s http://127.0.0.1:8765/v1/status
curl -s -X POST http://127.0.0.1:8765/v1/test -H 'Content-Type: application/json' -d '{"state":"thinking"}'
```

Project Cursor hooks in `.cursor/hooks.json` POST through `scripts/cursor-hook.sh`. They stay silent if the bridge is not running.

## Tests

```bash
cd desk-pet-bridge
pnpm test:rust
```

Firmware host tests remain:

```bash
./scripts/validate-tier0.sh
```

## BLE E2E (HEN-8)

1. Flash firmware `0.2.1-mac1` or later.
2. Open pairing from the tray or `POST /v1/pairing/start` (or serial `PAIR` / GPIO9 long-press).
3. Confirm the six-digit passkey on the TFT in the macOS prompt.
4. `GET /v1/status` should show `ble: connected` and the `DP-` name.
5. `POST /v1/test` should ACK and change the face.
6. Reboot the ESP32; the bridge should reconnect without pairing.
