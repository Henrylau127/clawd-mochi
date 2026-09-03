# Desk Pet architecture

Living design notes for the BLE-first firmware and macOS bridge. Product intent lives in [prd.md](prd.md). How to build and validate lives in [runbook.md](runbook.md).

## System overview

```text
Cursor hooks → desk-pet-bridge (menu bar + loopback HTTP)
                 │ encrypted GATT Command RX
                 ▼
            ESP32-C3 firmware ──► ST7789 face / wander / overlays
                 ▲
            USB serial STATUS / TEST / PAIR (recovery + agents)
```

- **No Wi-Fi** in production firmware. USB-C is power, flash, and serial only.
- Recovery Wi-Fi sketch (optional display check): `sketches/clawd_mochi_recovery/`.

## Firmware modes

| Mode | Behavior |
|------|----------|
| `WANDER` | Local idle / ambient screens (~15s cycle) while disconnected or session ended |
| `AGENT` | Coding / thinking / idle / waiting / error (+ attention / security overlays) |
| `SLEEP` | Backlight off |

Wander is intentional; it is not the original orange Wi-Fi Mochi controller UI. Agent expressions appear only in `AGENT` mode.

## BLE

- Advertise as `DP-<last4-MAC>` (e.g. `DP-CC98`) with the desk-pet service UUID.
- LE Secure Connections, bonding in NVS, encrypted/authenticated GATT for command / status / config.
- **120s pairing window** via serial `PAIR`, GPIO9 long-press, or bridge guidance — required for new centrals.
- During an open pairing window the link must stay up through passkey auth (do not disconnect on early insecure GATT).
- Bonded reconnect should not need another PIN.
- Passkey shown on TFT when authentication begins.

## Bridge

- Tauri v2 **Accessory** menu-bar app (`desk-pet-bridge/`); no Dock icon by design.
- CoreBluetooth via `btleplug`; loopback HTTP for Cursor hooks (port from Settings / `loopback_addr`, often `6900`).
- Secure session: encrypted status read + status subscription before `secure_session: ready`.
- Auto-reconnect on the active roster pet. Do not put a short unconditional sleep in the same `tokio::select!` as the reconnect timer (it cancels reconnect).
- Tray icon: corner status dot (green / yellow / red / grey) + hover tooltip.
- Mock mode: `DESKPET_BLE_MOCK=1` → in-process `DP-TEST` (no radio).

## Cursor hooks

```text
.cursor/hooks.json → scripts/cursor-hook.sh → POST /v1/cursor/event
```

- Thought hooks → thinking; tool/edit hooks (`preToolUse`, `postToolUse`, `afterFileEdit`, …) → **coding** so the face does not stick on thinking during edits.
- Hooks are silent if the bridge is not running.

## Key source trees

| Area | Path |
|------|------|
| Firmware entry / loop | `src/main.cpp` |
| BLE | `src/transport/` |
| Face / wander / extensions | `src/display/`, `src/extensions/` |
| Protocol / serial | `src/utils/` |
| Bridge BLE client | `desk-pet-bridge/src-tauri/src/ble/` |
| Bridge tray | `desk-pet-bridge/src-tauri/src/tray.rs` |

## Historical plan

The original Cursor implementation plan is archived at [archive/desk-pet-firmware-plan.md](archive/desk-pet-firmware-plan.md) (snapshot; may disagree with current code).
