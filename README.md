<!-- LOGO -->
<p align="center">
  <img src="pics/clawd_mochi_banner.png" alt="Clawd Mochi Logo" width="700"/>
</p>

# Clawd Mochi Desk Pet

A physical desk companion derived from the independent Clawd Mochi fan project. The current repository contains modular ESP32-C3 firmware, a 1.54-inch ST7789 display driver and face graphics, a BLE transport, serial diagnostics, host tests, and a macOS Tauri menu-bar bridge for Cursor events.

> This is an independent fan project. It is not affiliated with, sponsored by, or endorsed by Anthropic. “Claude” and “Clawd” are trademarks of Anthropic.

## Current architecture

The production firmware is BLE-first. It does not host a Wi-Fi access point, web server, or phone controller. USB-C is used for power, flashing, and serial recovery. The macOS bridge receives local Cursor hook events and sends compact JSON over authenticated, encrypted BLE.

- `WANDER` runs local idle, quote, and ambient screens while the pet is disconnected.
- `AGENT` displays coding, thinking, idle, waiting, and error states.
- `SLEEP` turns the backlight off.
- Attention escalation, quota display, and security-event overlays are implemented in `src/extensions/`.
- Firmware is organized under `src/`; it is not required to remain a single file.

See the [product requirements](docs/prd.md), [architecture](docs/architecture.md), [runbook](docs/runbook.md), and [STATUS](docs/STATUS.md) for design and current progress.

## Hardware and wiring

The reference build uses an **ESP32-C3 Super Mini** and an **ST7789 1.54-inch 240x240 SPI TFT**. Connect display power to **3.3V only**.

| ST7789 pin | ESP32-C3 Super Mini |
|------------|--------------------|
| VCC | 3V3 |
| GND | GND |
| SDA / MOSI | GPIO10 |
| SCL / SCK | GPIO8 |
| RES | GPIO2 |
| DC | GPIO1 |
| CS | GPIO4 |
| BL | GPIO3 |

The BOOT button is GPIO9 and a long press opens the temporary pairing window.

## PlatformIO and pioarduino

The project uses PlatformIO with the pioarduino ESP32 platform URL declared in [`platformio.ini`](platformio.ini). The default environment is `esp32-c3-super-mini`; the debug environment adds ASCII face previews and extra diagnostics.

Use the pioarduino PlatformIO CLI at `~/.platformio/penv/bin/pio` (or the pioarduino IDE extension):

```bash
~/.platformio/penv/bin/pio run -e esp32-c3-super-mini
~/.platformio/penv/bin/pio run -e esp32-c3-super-mini-debug
```

Run the native protocol tests:

```bash
~/.platformio/penv/bin/pio test -e native -f test_protocol
```

The no-hardware Tier 0 helper runs both firmware builds and the host test:

```bash
./scripts/validate-tier0.sh
```

## USB-C flashing and serial diagnostics

Connect the ESP32-C3 by a data-capable USB-C cable. On macOS, list native USB serial devices and upload with:

```bash
ls /dev/cu.usb*
~/.platformio/penv/bin/pio run -t upload -e esp32-c3-super-mini
~/.platformio/penv/bin/pio device monitor -b 115200
```

If the port is missing, unplug and reconnect the board. If upload does not start, hold **BOOT** while tapping reset and retry. The firmware uses native USB CDC at 115200 baud and prints a redacted boot banner with firmware version, device suffix, and heap information.

Useful commands in the serial monitor:

```text
STATUS
TEST coding
TEST thinking
TEST idle
TEST waiting
TEST error
TEST security
TEST celebrate
WANDER
PAIR
UNBOND
SLEEP
WAKE
HELP
```

`STATUS` returns one-line JSON; `PAIR` opens a 120-second pairing window, `UNBOND` clears stored bonds, and `WANDER` exits a forced agent state. `TEST celebrate` shows the done face and a short caption. Do not put passkeys, bond keys, tokens, or other sensitive data in logs. Caption text is a short allowlisted label; see [docs/scene-ui.md](docs/scene-ui.md).

For WSL2 USB passthrough, macOS setup, serial commands, and the bridge checklist, see the [runbook](docs/runbook.md).

## BLE-first security

The device advertises a generic name in the form **`DP-<last4-MAC>`**, such as `DP-A1B2`. The suffix is a troubleshooting identifier; the advertisement does not contain user, project, Cursor, quota, security-event, or Wi-Fi data.

An explicit 120-second pairing window can be opened from the bridge, with serial `PAIR`, or with a
GPIO9 long press. NimBLE uses LE Secure Connections, authenticated/encrypted GATT permissions, and
NVS bond persistence. The Mac bridge performs a secure GATT handshake before accepting the
connection; the TFT displays the passkey whenever a new authentication attempt begins. Command,
configuration, and status access remain protected, and a successful bond should reconnect without
another PIN.

## macOS Tauri bridge

The [`desk-pet-bridge/`](desk-pet-bridge/) directory contains a Tauri v2 menu-bar app. It scans for `DP-*`, connects through macOS CoreBluetooth via `btleplug`, queues events while reconnecting, and forwards Cursor hooks from `.cursor/hooks.json`.

Install and run it on macOS:

```bash
./scripts/check-mac-env.sh
cd desk-pet-bridge
pnpm install
pnpm tauri dev
```

Accept the macOS Bluetooth permission prompt. For first enrollment, open `PAIR` over USB or hold
GPIO9 on the pet, then use **Settings → Add pet** and confirm the passkey shown on the TFT. The
passkey is only needed for the initial bond. To remove the macOS bond, use System Settings >
Bluetooth > the pet > **Forget This Device**; the bridge cannot invoke CoreBluetooth unpairing.

The bridge API binds only to loopback:

```bash
curl -s http://127.0.0.1:8765/v1/status
curl -s -X POST http://127.0.0.1:8765/v1/test \
  -H 'Content-Type: application/json' \
  -d '{"state":"coding"}'
```

It also supports `POST /v1/cursor/event`, `POST /v1/power`, `POST /v1/pairing/start`, and `POST /v1/pairing/stop`. Pairing endpoints provide enrollment guidance; they do not remotely authorize a new bond. The full API and bridge behavior are documented in [`desk-pet-bridge/README.md`](desk-pet-bridge/README.md).

Without hardware, run the in-process `DP-TEST` BLE mock:

```bash
./scripts/validate-host.sh
cd desk-pet-bridge
DESKPET_BLE_MOCK=1 pnpm tauri dev
curl -s http://127.0.0.1:8765/v1/status
```

Mock mode validates the bridge and loopback API but does not validate CoreBluetooth, radio pairing, the TFT, or physical hardware.

## Recovery sketch

If the modular firmware or wiring needs a known-good display check, flash [`sketches/clawd_mochi_recovery/clawd_mochi.ino`](sketches/clawd_mochi_recovery/clawd_mochi.ino) with Arduino IDE 2 on Windows. This recovery-only sketch preserves the original Clawd Mochi Wi-Fi soft AP and web canvas; production firmware does not.

## Validation tiers and blockers

Validation is intentionally staged:

1. **Tier 0, no hardware:** PlatformIO firmware builds and native `test_protocol` pass.
2. **Tier 1, USB:** flash succeeds and the serial boot banner appears.
3. **Tier 2, USB:** `STATUS`, `TEST`, pairing, bond, sleep, wake, and help diagnostics work.
4. **Tier 3, BLE:** advertise, pair, encrypted writes, and reboot reconnect (phone nRF Connect or Mac BLE / bridge).
5. **Tier 4, manual display:** a person confirms TFT expressions and overlays match the reported state.
6. **Tier 5, Mac BLE:** CoreBluetooth pairing, encrypted GATT writes, reconnect, bridge API, and Cursor hooks.

See [STATUS.md](docs/STATUS.md) for the current done/open list and the [runbook](docs/runbook.md) for procedures.

## Case and project context

The original project includes display and media assets under [`pics/`](pics/). A reference case is also available on [MakerWorld](https://makerworld.com/en/models/2559505-clawd-mochi-physical-claude-code-mascot#profileId-2820000). The original Clawd Mochi graphics remain the visual starting point for the modular firmware.

Contributions are welcome, including new local animations, screens, sensors, and bridge adapters. Keep changes aligned with the documented BLE-first architecture and validation tiers.

## License

Source code is licensed under the [MIT License](LICENSE). 3D models and media assets are licensed under **CC BY-NC-SA 4.0** where applicable.
