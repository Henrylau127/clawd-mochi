# Desk Pet runbook

Build, flash, serial, BLE bridge, and Cursor hooks. Living status is in [STATUS.md](STATUS.md). Design is in [architecture.md](architecture.md).

## Docs map

| Doc | Use when |
|-----|----------|
| [prd.md](prd.md) | Product requirements |
| [architecture.md](architecture.md) | How the system fits together |
| [STATUS.md](STATUS.md) | Done / open checklist |
| [archive/](archive/) | Old session notes and the original plan snapshot |

---

## Tier ladder

| Tier | Where | Proves |
|------|--------|--------|
| 0 | Any host, no USB | Firmware builds + native `test_protocol` |
| 1 | USB | Flash + boot banner |
| 2 | USB | Serial `STATUS` / `TEST` / `PAIR` / … |
| 3 | Phone or Mac BLE | Advertise, pair, encrypted access, reconnect |
| 4 | Human + TFT | Expressions/colors match state |
| 5 | Mac + bridge | Tauri bridge, CoreBluetooth, Cursor hooks E2E |

---

## Common commands (any host)

```bash
./scripts/validate-tier0.sh
~/.platformio/penv/bin/pio run -e esp32-c3-super-mini
~/.platformio/penv/bin/pio test -e native -f test_protocol
```

---

## WSL2 flash / serial

### Prerequisites

- pioarduino / PlatformIO CLI at `~/.platformio/penv/bin/pio`
- [usbipd-win](https://github.com/dorssel/usbipd-win) on Windows
- User in **`dialout`** (`./scripts/check-wsl-serial.sh`)

```bash
sudo usermod -aG dialout $USER   # once, then re-login or newgrp dialout
```

### usbipd (Windows Admin PowerShell)

```powershell
usbipd list
usbipd bind --busid <BUSID>
usbipd attach --wsl --busid <BUSID>
```

```bash
ls /dev/ttyACM*
~/.platformio/penv/bin/pio run -t upload -e esp32-c3-super-mini
~/.platformio/penv/bin/pio device monitor -b 115200
```

Hold **BOOT** and tap reset if upload does not start.

---

## macOS flash / serial

```bash
./scripts/check-mac-env.sh
ls /dev/cu.usb*
~/.platformio/penv/bin/pio run -t upload -e esp32-c3-super-mini
~/.platformio/penv/bin/pio device monitor -b 115200
```

---

## Serial diagnostics (Tier 2)

| Command | Behavior |
|---------|----------|
| `STATUS` | One-line JSON (`mode`, `state`, `expression`, `fw`, `secure_session`, …) |
| `TEST coding\|thinking\|idle\|waiting\|error\|security` | Force agent expression |
| `WANDER` | Leave AGENT; redraw wander |
| `PAIR` / `UNBOND` | Pairing window / clear bonds |
| `SLEEP` / `WAKE` | Sleep backlight; wake only from SLEEP |
| `HELP` | List commands |

`thinking` has **no auto-timeout**. Clear with `WANDER`, `sessionEnd`, or another `TEST` / hook state. `WAKE` does not leave AGENT mode.

Debug build prints `[face] <expression>` after draws:

```bash
~/.platformio/penv/bin/pio run -t upload -e esp32-c3-super-mini-debug
```

---

## Graphics notes

Wander cycles **idle eyes** and **ambient stars** — that is firmware design, not a regression vs the original Wi-Fi sketch. Orange wiggle-eyes live only in the recovery sketch / would need a separate firmware feature.

---

## macOS bridge (Tier 5)

```bash
./scripts/check-mac-env.sh
./scripts/validate-host.sh          # no hardware: firmware tests + mock
cd desk-pet-bridge
pnpm install
pnpm tauri dev                      # real BLE
# DESKPET_BLE_MOCK=1 pnpm tauri dev  # DP-TEST mock
```

### Loopback port

Check Settings or:

```bash
curl -s http://127.0.0.1:6900/v1/status   # often 6900 on this Mac; else 8765
```

Use the `loopback_addr` from status for hooks and curls.

### First enrollment (Add pet)

1. Power/flash the ESP32 so it advertises `DP-*`.
2. Open pairing: serial `PAIR` or GPIO9 long-press (TFT passkey).
3. Bridge **Settings → Add pet**; accept macOS passkey prompt.
4. Expect `ble: connected`, `secure_session: ready`, roster name like `DP-CC98`.

With **no** advertising pet, Add pet correctly enrolls nothing.

After **Remove pet**, forget the device in System Settings → Bluetooth (bridge cannot unpair).

### Checklist

1. Menu-bar popover opens; tray dot reflects state (green only when link verified).
2. Bluetooth permission accepted.
3. Status shows **VERIFYING** briefly, then **CONNECTED** + `link_verified: true` after encrypted status read.
4. `POST /v1/test` with `{"state":"coding"}` changes the face.
5. Cursor hooks POST to the same loopback port.
6. Kill/restart bridge: auto-reconnect without a new PIN (~30–60s cold CB is OK). **Reconnect** is optional.
7. Idle link: bridge reads encrypted status every 30s (keepalive; first check ~5s). Firmware uses 20s supervision timeout once bonded (flash required).
8. After **firmware flash**, if popover shows VERIFYING or writes do nothing, click **Reconnect** (zombie CoreBluetooth session).

API details: [desk-pet-bridge/README.md](../desk-pet-bridge/README.md).

### Cursor hooks

`.cursor/hooks.json` → `scripts/cursor-hook.sh` → `/v1/cursor/event`.  
Thought → thinking; tool/edit hooks → coding. Reload Cursor hooks after changing the map.

Agent skill (optional): `~/.cursor/skills-cursor/desk-pet-cursor-hooks/SKILL.md`.

### Codex hooks

OpenAI Codex loads `.codex/hooks.json` when this project layer is trusted, and `~/.codex/hooks.json` for every session. This repo ships the project file. It runs `scripts/agent-hook.sh`, which posts to `POST /v1/codex/event`.

1. Use a Codex build with hooks ([hook docs](https://learn.chatgpt.com/docs/hooks)). Leave `[features] hooks` at the default `true`.
2. Review and trust the hook commands with `/hooks` (desktop: Settings → Hooks). Codex skips untrusted hooks.
3. Start the bridge. Hooks stay silent if it is down.
4. Start or resume a Codex session in this repo.

Session start shows coding, model requests show thinking, tool use stays on coding, an approval prompt or finished turn shows waiting, a failed model request shows error, an interrupt shows idle, and session end returns to wander. The script reads only the `PostModelRequest` status enum from stdin. Prompts, paths, commands, and error text are discarded.

For other repos, point `~/.codex/hooks.json` at this checkout's `scripts/agent-hook.sh` with the same arguments.

### OMP hooks

OMP here is the oh-my-pi CLI (`omp`, [hooks](https://omp.sh/docs/hooks)), not a separate OpenAI product. Codex is OpenAI's hookable agent. oh-my-pi discovers `.omp/hooks/post/desk-pet.js` and that file calls the same shell script with an event name plus `ok`, `error`, `done`, or `continue`.

1. Run `omp` from this repo, or copy the module to `~/.omp/agent/hooks/post/` and set `DESKPET_HOOK_SCRIPT` to this repo's `scripts/agent-hook.sh`.
2. Start a new omp session so discovery reloads.
3. Start the bridge.

`source` on the pet event is `omp`. Firmware ignores that key today. This adapter does not send caption or usage strings. Claude Code and upstream Pi stay disabled until they get their own mappers.

---

## Recovery sketch

If wiring/display is suspect, flash `sketches/clawd_mochi_recovery/clawd_mochi.ino` from Arduino IDE on Windows (original Wi-Fi soft AP). Production firmware stays BLE-first.

---

## Optional MCP

Do not install MCP servers without explicit approval. Serial/BLE MCP (if approved) can automate Tier 2 / BLE checks against an allowlisted port or adapter.
