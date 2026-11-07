# Desk Pet Bridge (macOS)

Tauri v2 menu-bar app that forwards Cursor agent events to the ESP32 desk pet over encrypted BLE.

## What it does

- Lives in the macOS menu bar (no dock icon).
- Menu-bar icon shows a corner status dot (green connected, yellow connecting/securing, red offline, grey unpaired) with a hover tooltip.
- Scans for `DP-<MAC>` and the desk-pet GATT service.
- Writes compact JSON to the encrypted Command RX (`7d3f1b2e-...d002`) using CoreBluetooth via `btleplug`.
- Performs a secure GATT handshake before reporting the pet as connected; the first pairing can wait for the macOS passkey prompt.
- Exposes a loopback HTTP API on `127.0.0.1:8765` for Cursor, Codex, and OMP hooks.
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

Tray actions: click the menu-bar icon to open the status popover (not a dropdown menu). Click outside or press Escape to dismiss. Use **Quit** in the popover footer.

macOS will prompt for Bluetooth access. Initial enrollment is device-confirmed: open `PAIR` in the
USB serial monitor or hold GPIO9 on the pet, then click **Add pet**. The bridge performs a secure
GATT handshake; the ESP32 displays the passkey on the TFT and macOS asks for it. The resulting bond
is stored by macOS and the ESP32, so later bridge restarts should reconnect without another PIN.
The bridge cannot remove a CoreBluetooth bond; use System Settings > Bluetooth > the pet >
**Forget This Device** when a full reset is needed.

## Loopback API

Bound only to `127.0.0.1:8765`.

| Method | Path | Body |
|--------|------|------|
| POST | `/v1/cursor/event` | Cursor hook or desk-pet JSON |
| POST | `/v1/codex/event` | Codex hook name or desk-pet JSON |
| POST | `/v1/omp/event` | OMP event name or desk-pet JSON |
| POST | `/v1/test` | `{"state":"coding"}` |
| GET | `/v1/status` | — |
| POST | `/v1/power` | `{"action":"sleep"}` or `{"action":"wake"}` |
| POST | `/v1/pairing/start` | — |
| POST | `/v1/pairing/stop` | — |

Pairing controls only start or stop the bridge's enrollment guidance. They do not remotely authorize
a new device; use serial `PAIR` or a GPIO9 long-press on the pet.

Example:

```bash
curl -s http://127.0.0.1:8765/v1/status
curl -s -X POST http://127.0.0.1:8765/v1/test -H 'Content-Type: application/json' -d '{"state":"thinking"}'
```

Project Cursor hooks in `.cursor/hooks.json` POST through `scripts/cursor-hook.sh`. Codex (`.codex/hooks.json`) and oh-my-pi OMP (`.omp/hooks/post/desk-pet.js`) POST through `scripts/agent-hook.sh` to their own routes. Hooks stay silent if the bridge is not running. Enablement steps are in [docs/runbook.md](../docs/runbook.md). The pet event `source` is `cursor`, `codex`, or `omp`. Adapters send states only, not captions or usage text.

## Tests without hardware

The ESP32 is not required for protocol or bridge tests. A simulated `DP-TEST` peripheral implements the same JSON events as firmware:

```bash
./scripts/validate-host.sh
```

To run the menu-bar app against the simulator instead of CoreBluetooth:

```bash
cd desk-pet-bridge
DESKPET_BLE_MOCK=1 pnpm tauri dev
curl -s http://127.0.0.1:8765/v1/status
# ble=connected, device=DP-TEST
```

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

1. Flash firmware `0.2.2-mac2` or later.
2. Start device pairing with serial `PAIR` or a GPIO9 long-press.
3. Open **Add pet** from Settings while the pairing window is open.
4. Confirm the six-digit passkey shown on the TFT in the macOS prompt.
5. `GET /v1/status` should show `ble: connected` and the `DP-` name.
6. `POST /v1/test` should ACK and change the face.
7. Restart the bridge or reboot the ESP32; the bond should reconnect without another PIN.
