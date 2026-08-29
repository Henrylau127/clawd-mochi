# WSL validation guide

This document describes how to build, flash, and validate the Desk Pet firmware on WSL2 before a Mac bridge is available. For the full firmware roadmap and milestone plan, see [desk-pet-firmware-plan.md](desk-pet-firmware-plan.md).

## Prerequisites

- pioarduino IDE extension or PlatformIO CLI at `~/.platformio/penv/bin/pio`
- Windows host with [usbipd-win](https://github.com/dorssel/usbipd-win) for USB passthrough
- ESP32-C3 Super Mini connected over native USB (`303A:1001`)
- WSL user in the **`dialout`** group (required for Tier 1 upload and Tier 2 serial monitor)

### dialout group (required for upload/monitor)

`/dev/ttyACM0` is owned by `root:dialout`. Without membership you get `Permission denied` on upload.

Check:

```bash
groups | grep -q dialout && echo "dialout ok" || echo "dialout missing"
ls -l /dev/ttyACM0
```

Fix (run once in WSL, then **log out and back in** or `newgrp dialout`):

```bash
sudo usermod -aG dialout $USER
newgrp dialout
```

Quick permission test:

```bash
test -w /dev/ttyACM0 && echo "ttyACM0 writable"
```

Or run the repo helper:

```bash
./scripts/check-wsl-serial.sh
```

## usbipd workflow (Windows Admin PowerShell)

```powershell
usbipd list
usbipd bind --busid <BUSID>
usbipd attach --wsl --busid <BUSID>
```

In WSL, confirm the device appears:

```bash
ls /dev/ttyACM*
```

If upload fails, hold **BOOT** (GPIO9) and tap reset while starting upload.

## Build commands

```bash
cd /home/henrylau/projects/clawd-mochi

# Tier 0: compile (no hardware)
~/.platformio/penv/bin/pio run -e esp32-c3-super-mini
~/.platformio/penv/bin/pio run -e esp32-c3-super-mini-debug

# Tier 0: host unit tests
~/.platformio/penv/bin/pio test -e native -f test_protocol
```

Record flash/RAM usage from the build summary (`RAM`, `Flash` columns).

**Recorded build (`esp32-c3-super-mini`):**

| Build | RAM | Flash | FW |
|-------|-----|-------|-----|
| Tier 0 baseline (2026-08-30) | 16,108 B (4.9%) | 397,904 B (30.4%) | `0.1.0-tier2` |
| Steps 1–6 + BLE/extensions (2026-08-30) | 24,864 B (7.6%) | 650,957 B (49.7%) | `0.2.0-tier6` |

## Upload and monitor (Tier 1)

```bash
~/.platformio/penv/bin/pio run -t upload -e esp32-c3-super-mini
~/.platformio/penv/bin/pio device monitor -b 115200
```

Expected boot banner:

```text
Desk Pet fw=0.1.0-tier2 device=DP-XXXX heap=...
Type HELP for serial commands.
```

## Serial diagnostics (Tier 2)

| Command | Expected behavior |
|---------|-------------------|
| `STATUS` | One-line JSON: `mode`, `state`, `expression`, `backlight`, `ble_connected`, `pairing_active`, `fw` |
| `TEST coding` | Forces agent coding expression; replies `OK coding` |
| `TEST thinking` | Forces thinking expression |
| `TEST idle` | Forces idle expression |
| `TEST waiting` | Forces waiting expression |
| `TEST error` | Forces error expression |
| `TEST security` | Forces security expression |
| `WANDER` | Leave AGENT mode and return to wander cycling; replies `OK wander` |
| `PAIR` | Opens pairing window flag (`pairing_active: true` in STATUS) |
| `UNBOND` | Clears bond/pairing flags |
| `SLEEP` | Enters sleep mode, backlight off |
| `WAKE` | Wake from **SLEEP** only; replies `ERR not sleeping` if already awake |
| `HELP` | Lists commands |

Example:

```bash
# In pio device monitor, type:
STATUS
TEST coding
STATUS
```

Example STATUS output:

```json
{"mode":"agent","state":"coding","expression":"coding","backlight":100,"ble_connected":false,"pairing_active":false,"fw":"0.1.0-tier2"}
```

### Stuck on `><` (thinking) face

AGENT `thinking` has **no auto-timeout**. It usually means `TEST thinking` (or a JSON `stateUpdate` with `thinking`) ran without a follow-up clear.

| Goal | Serial command |
|------|----------------|
| Return to wander cycling | `WANDER` or `{"event":"sessionEnd"}` |
| Idle eyes but stay in AGENT | `TEST idle` |
| Check current mode | `STATUS` |

`WAKE` does **not** clear AGENT mode — only exits SLEEP.

After firmware with wander redraw fix (see [graphics-and-cursor.md](graphics-and-cursor.md)), `WANDER` / `sessionEnd` immediately redraws wander screens instead of leaving the old face on screen.

### Debug build ASCII preview

Build/upload the debug environment for extra serial logging:

```bash
~/.platformio/penv/bin/pio run -t upload -e esp32-c3-super-mini-debug
```

After each face draw, serial prints `[face] <expression>`.

## Validation tier checklist

| Tier | Requirement | Pass criteria |
|------|-------------|---------------|
| 0 | No USB | `pio run` succeeds; `pio test -e native` passes; flash/RAM recorded |
| 1 | WSL + usbipd | Upload succeeds; boot banner on serial monitor |
| 2 | WSL + usbipd | `STATUS` JSON + `TEST`/`PAIR`/`UNBOND`/`SLEEP`/`WAKE`/`HELP` work |
| 3 | Phone | nRF Connect: `DP-<MAC>` advertise, pair, encrypted write, reboot reconnect |
| 4 | Human | TFT expressions match state (manual) |
| 5 | Mac | Tauri bridge E2E (deferred) |

### Validation log

| Date | Tier | Result | Notes |
|------|------|--------|-------|
| 2026-08-30 | 0 | PASS | `pio run`, `pio test -e native` 8/8, flash 49.7% / RAM 7.6% |
| 2026-08-30 | 1 | PASS | Upload to `/dev/ttyACM0` via `sg dialout` / dialout group |
| 2026-08-30 | 2 | PASS | `STATUS`, `TEST coding`, `PAIR`, `HELP` via pyserial — fw `0.1.0-tier2` |
| 2026-08-30 | 2 | PASS | Re-check after steps 1–6 — fw `0.2.0-tier6`, `TEST thinking` OK |
| 2026-08-30 | 3 | PENDING | BLE stack implemented (NimBLE 2.5); needs nRF Connect phone test |
| — | 4 | Not run | Manual TFT check |

## Optional: serial-mcp for agents

An MCP server such as [qarnet/serial-mcp](https://github.com/qarnet/serial-mcp) can automate Tier 2 by sending `STATUS\n` and asserting JSON.

Do **not** install or configure MCP servers without explicit user approval (MCP governance). If approved, example `.cursor/mcp.json` snippet:

```json
{
  "mcpServers": {
    "serial": {
      "command": "serial-mcp",
      "args": ["--allowlist=/dev/ttyACM0"]
    }
  }
}
```

Example agent flow: `transact("STATUS\n")` then parse JSON fields.

## Recovery sketch

If hardware wiring is suspect, flash the original Wi-Fi recovery sketch from Arduino IDE on Windows:

`sketches/clawd_mochi_recovery/clawd_mochi.ino`

This preserves the original `ClaWD-Mochi` soft AP and web controller for display testing.

## Graphics vs original Mochi + Cursor hooks

See [docs/graphics-and-cursor.md](graphics-and-cursor.md) for why wander mode shows idle/ambient screens (ambient uses random star positions with a subtle twinkle), and how to wire Cursor hooks once the Mac Tauri bridge exists.

Agent setup skill: `~/.cursor/skills-cursor/desk-pet-cursor-hooks/SKILL.md`
