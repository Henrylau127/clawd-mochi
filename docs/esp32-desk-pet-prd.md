# ESP32-C3 Desk Pet + Agent Status Display (v5)

## Product Overview

A 1.54" 240×²⁴⁰ ST7789-based desk pet that:
- Shows **animated graphics** based on the original **Clawd Mochi** firmware (face/eyes, expressions).
- Acts as a **Cursor agent status display** (coding, thinking, idle, waiting_for_user, error).
- Wanders through fun screens (weather, stocks/crypto, quotes, animations) when no agent session is active.
- **Demands attention** with background color changes / flashing if it needs user interaction and the user hasn't responded for a while.
- Reacts to **high-sensitivity Cursor events** (SSH logins, GitHub logins, security-related actions) that are easy to miss.
- Uses **Bluetooth Low Energy (BLE)** to communicate with a small macOS **Tauri menu-bar bridge**; Wi-Fi is not required for agent integration.

Target hardware:
- ESP32-C3 Super Mini
- ST7789 1.54" 240×²⁴⁰ TFT (SPI)

Target software:
- ESP32 firmware primary development: **Cursor + PlatformIO**
- ESP32 firmware fallback: Arduino IDE 2
- macOS bridge: **Tauri v2**, Rust backend, lightweight menu-bar-only app

Base firmware:
- **Original Clawd Mochi** (`yousifamanuel/clawd-mochi`) as the starting point, then extend with BLE transport, Cursor agent integration, wander screens, attention system, and security events.

---

## Goals

1. **Agent-aware desk pet**: visually reflect Cursor agent states with expressive graphics.
2. **BLE-first office operation**: work without ESP32 Wi-Fi in locked-down office networks; use USB only for power, flashing, and serial recovery.
3. **BLE access control**: the device may be discoverable in nearby BLE scans, but only the bonded/encrypted Mac can send commands or subscribe to device status.
4. **Attention system**: escalate visually for unacknowledged approvals, questions, and failures.
5. **Security / high-sensitivity events**: visibly surface SSH/GitHub logins, privileged actions, and deployments without exposing credentials or sensitive details.
6. **Wander mode**: cycle through fun local screens whenever agent mode is inactive.
7. **Cursor quota display**: show a compact color-coded usage bar.

---

## User Stories

### Agent Status & Graphics

- As a developer, I want the device to show animated eyes/face that reflect Cursor agent state, so I can understand activity at a glance.
- As a developer, I want expressions for `coding`, `thinking`, `idle`, `waiting_for_user`, and `error`.
- As a developer, I want state-specific colors: blue coding, yellow thinking, green idle, orange waiting, red error.

### BLE Privacy and Access

- As a developer in a shared office, I accept that nearby devices can discover the BLE peripheral while it advertises, but I want unbonded devices unable to write commands, read meaningful status, or subscribe to notifications.
- As a developer, I want the peripheral name to be generic and non-descriptive, rather than identifying it as a desk pet or agent-status device.
- As a developer, I want pairing to occur only when I explicitly enable a short pairing window from the device/Mac bridge; normal operation should accept only the previously bonded Mac.

### Attention System

- As a developer, I want the device to escalate visually if the agent needs attention and I have not responded: urgent color after 2 minutes, slow pulse after 5 minutes, faster pulse and distinct expression after 10 minutes.

### Security / High-Sensitivity Events

- As a developer, I want a distinct security alert for SSH logins, GitHub login/new-device activity, privileged actions, and deployments, without transmitting credentials, tokens, command arguments, or sensitive prompt text.

### Wander Mode

- As a developer, I want fun screens when no agent session is active: animated face, optional Mac-provided weather/market data, quotes/facts, and ambient animations.

### Mac Menu Bar Bridge

- As a developer, I want a minimal Tauri menu-bar app that shows BLE connection state, handles reconnects, and forwards Cursor events while no window is open.
- As a developer, I want tray actions: `Reconnect`, `Start pairing`, `Stop pairing`, `Test pet`, `Show status`, and `Quit`.

---

## Functional Requirements

### FR1: BLE Transport and Identifiers

The ESP32 shall advertise with a generic, non-descriptive name:

- Device name: `DP-<last4-MAC>` (example: `DP-CC98`)
- Do not advertise names such as `DeskPet`, `Mochi`, `Cursor`, `Claude`, `Codex`, or `Agent`.
- The suffix is derived from the last four hexadecimal characters of the ESP32 base MAC and is for troubleshooting only.

The firmware shall expose a custom vendor-specific BLE GATT service. Custom 128-bit UUIDs are appropriate for a non-standard service.

| Item | UUID | Properties | Direction | Purpose |
|---|---|---|---|---|
| Desk Pet service | `7d3f1b2e-5a8c-4f61-9e09-9a7d4955d001` | N/A | N/A | Primary custom service |
| Command RX | `7d3f1b2e-5a8c-4f61-9e09-9a7d4955d002` | Write Encrypted + Authenticated | Mac → ESP32 | JSON command/event payloads |
| Status TX | `7d3f1b2e-5a8c-4f61-9e09-9a7d4955d003` | Notify Encrypted + Authenticated; Read Encrypted | ESP32 → Mac | ACKs, state, firmware version |
| Config RX | `7d3f1b2e-5a8c-4f61-9e09-9a7d4955d004` | Write Encrypted + Authenticated | Mac → ESP32 | Optional configuration payloads |

The ESP32 shall advertise the custom service UUID but shall not expose unprotected command/status characteristics. GATT access permissions can require encryption and authorization/authentication for characteristic reads and writes.

#### BLE Command Protocol

- UTF-8 JSON, one object per BLE write.
- Keep standard event payloads below 180 bytes initially; implement fragmentation only if future requirements need it.
- ESP32 returns an ACK/error on Status TX if notifications are enabled.

Example command:

```json
{
  "id": "e69e701a",
  "event": "sessionStart",
  "state": "coding",
  "source": "cursor",
  "ts": "2026-08-27T01:10:00+08:00"
}
```

Example ACK:

```json
{
  "id": "e69e701a",
  "ok": true,
  "mode": "agent",
  "state": "coding"
}
```

### FR2: BLE Security, Pairing, and Privacy

#### Security requirements

- Use **LE Secure Connections**, bonding, and authenticated/encrypted GATT access.
- The command, config, and status characteristics must require encrypted and authenticated access (MITM protection where supported by the selected stack).
- Store bond keys in ESP32 NVS so the trusted Mac reconnects without manual pairing after reboot.
- On subsequent connections, the device must reject unencrypted/unbonded writes and notification subscriptions.
- Mac bridge must fail closed: do not downgrade to an unencrypted transport if pairing/security fails.

ESP-IDF/NimBLE supports characteristic permissions that require encryption/authorization, and bonding stores key material for later encrypted reconnection.

#### Pairing mode

- Default state after provisioning: **pairing disabled**.
- Only the already bonded Mac is permitted to access protected characteristics.
- A new pairing window can be started only by:
  - Tauri tray action `Start pairing`, which sends a command through the existing trusted connection; or
  - A physical BOOT/button gesture on the ESP32 as a recovery mechanism; or
  - A temporary serial recovery command over USB.
- Pairing window duration: 120 seconds, then automatically disable pairing.
- During pairing, show a distinct local display screen/animation (e.g., lock icon + six-digit code) so the user can verify that pairing is intentional.
- Use a randomly generated 6-digit passkey per pairing window when the selected BLE stack and macOS pairing UX support passkey entry/display. BLE passkeys are six digits.
- If the chosen NimBLE/macOS path cannot provide usable passkey confirmation because the ESP32 has no physical input and the display-only capability is insufficient, use bonded LE Secure Connections initially, record this limitation, and add a second application-layer enrollment secret before enabling command writes.

#### Bond management

- Support a local USB recovery command and/or physical gesture to erase bond data.
- Erasing bond data returns device to pairing-disabled unprovisioned state; user must explicitly open pairing mode to enroll a new Mac.
- Tauri bridge should expose `Forget device` only after a confirmation prompt.
- Never automatically accept a different central simply because no current connection exists.

#### Visibility and advertising behavior

- BLE visibility is expected while advertising; pairing does not make a peripheral inherently invisible.
- When connected to the trusted Mac, stop normal connectable advertising if stable reconnect behavior allows; otherwise use a slower advertising interval.
- When disconnected, advertise at an appropriately conservative interval so the trusted bridge can reconnect.
- Never encode user identity, project name, Cursor state, quota, security event contents, or Wi-Fi/network details in the advertisement payload.

### FR3: Agent Event API

The ESP32 firmware shall accept agent events through BLE Command RX:

```json
{
  "event": "sessionStart" | "sessionEnd" | "stateUpdate" | "securityEvent" | "attentionRequired" | "attentionCleared",
  "state": "coding" | "thinking" | "idle" | "waiting_for_user" | "error",
  "details": {
    "type": "ssh_login" | "github_login" | "deployment" | "sudo" | "model_error" | "no_capacity" | "no_internet",
    "message": "Optional non-sensitive human-readable message"
  }
}
```

Behavior:
- `sessionStart` → enter `AGENT` mode and set initial state.
- `stateUpdate` → update agent state.
- `sessionEnd` → return to `WANDER` mode.
- `securityEvent` → show security alert overlay/screen.
- `attentionRequired` → start attention timer.
- `attentionCleared` → clear escalation and resume current agent state.

### FR4: State Machine

Modes:
- `WANDER`: no active agent session.
- `AGENT`: active agent session.
- `SLEEP`: Mac absent, scheduled sleep, or manual sleep.

Transitions:
- `WANDER` → `AGENT` on `sessionStart`.
- `AGENT` → `WANDER` on `sessionEnd`.
- Any → `SLEEP` after configured Mac/BLE absence or explicit command.
- `SLEEP` → `WANDER` after reconnect or explicit wake.

### FR5: Graphics & Expressions

The firmware shall render a face with two animated eyes and mouth. Use original **Clawd Mochi** expressions/layout where possible.

Colors:
- coding blue-ish; thinking yellow-ish; idle green-ish; waiting orange-ish; error red-ish; security purple/red pattern.

Expressions:
- coding focused; thinking squint/looking; idle relaxed/smile; waiting wide/expectant; error X eyes/wavy mouth; security shield/lock + alert expression.

### FR6: Attention Escalation

On `attentionRequired`:
- 2 minutes: urgent background.
- 5 minutes: slow pulse (~1-second cycle).
- 10 minutes: faster pulse (~0.5-second cycle) plus attention expression.

Reset on `attentionCleared`, new `sessionStart`, or `sessionEnd`.

### FR7: Security Events

On `securityEvent`:
- Show a distinct security alert for 10–30 seconds.
- Use purple/red background, lock/shield graphic, concise type label (`SSH login`, `GitHub login`).
- Do not show credentials, tokens, hostnames, repositories, command arguments, raw prompts, or personally sensitive details by default.

### FR8: Wander Screens

In `WANDER` mode, cycle through:
- `PetIdleScreen`: original Mochi-like face, blink/look animations.
- `WeatherScreen`: optional Mac-provided weather.
- `StocksScreen`: optional Mac-provided market data.
- `QuoteScreen`: local quote/joke/fact set.
- `AmbientScreen`: stars, rain, bouncing logo, etc.

Default cycle: 15 seconds, configurable.

The ESP32 remains BLE-first; network-fed content should come from the Mac bridge where required, rather than relying on office Wi-Fi.

### FR9: Sleep & Backlight

- Backlight: normal 100%, dim 10–20%, off 0%.
- `SLEEP`: backlight off; deep sleep is optional after BLE wake/reconnect is proven.
- Mac bridge sends explicit sleep/wake according to Mac sleep/wake status or user choice.

### FR10: Cursor Quota Display

- Mac bridge obtains quota from configured Open Usage source and sends compact values over encrypted BLE.
- Firmware renders color-coded bar: green >50%, yellow 20–50%, red <20%.

---

## Non-Functional Requirements

- Agent state updates appear within 1 second of bridge receipt while BLE is connected.
- BLE reconnection must be automatic and non-blocking.
- ESP32 must continue stable wander mode while BLE is disconnected.
- Do not redraw full screen in main loop; use incremental/dirty-region rendering to prevent visible flashing.
- Handle malformed JSON, disconnected BLE, unavailable Cursor/Open Usage data, and rejected security attempts without crashing.
- Keep firmware assets lightweight: original Mochi-style vector drawing/compact bitmaps over large raster assets.
- Do not log sensitive data in Tauri or ESP32 logs.

---

## Technical Design

### Hardware

- ESP32-C3 Super Mini
- ST7789 1.54" 240×²⁴⁰ TFT (SPI)
- Verified original Clawd Mochi wiring:
  - SCLK → GPIO8
  - MOSI → GPIO10
  - CS → GPIO4
  - DC → GPIO1
  - RST → GPIO2
  - BLK → GPIO3
  - VCC → 3V3
  - GND → GND

### ESP32 Software Stack

- Primary: PlatformIO + Arduino core for ESP32 in Cursor.
- Fallback: Arduino IDE 2 recovery/display tests.
- Base: original **Clawd Mochi** `clawd_mochi.ino`.
- BLE: NimBLE-Arduino or ESP-IDF NimBLE; prefer NimBLE for memory efficiency.
- JSON: ArduinoJson.
- Keep original Mochi display implementation/library at first; refactor only after baseline works.

### macOS Bridge Stack

- Tauri v2 menu-bar-only app.
- Rust: `btleplug` BLE central/client (macOS CoreBluetooth), `axum`/`hyper` loopback server, `tokio`, `serde`, `serde_json`.
- Minimal tray UI: connection state, pairing action, reconnect, test pet, logs, quit.

Tauri v2 has system-tray APIs in Rust/JS suitable for a menu-bar-only bridge.

### macOS Privacy Configuration

Packaged app includes Bluetooth usage description:

```xml
<key>NSBluetoothAlwaysUsageDescription</key>
<string>Desk Pet Bridge uses Bluetooth to send Cursor status to your desk display.</string>
```

macOS requires Bluetooth permission for apps using CoreBluetooth.

---

## Architecture

### ESP32 Firmware: Extend Original Mochi

```text
src/
  clawd_mochi_base/        # Adapted from original clawd_mochi.ino
    Display.cpp/.h         # ST7789 init, SPI, backlight
    Face.cpp/.h            # Original-style eyes, mouth, expressions
    States.cpp/.h          # Original behavior + extension points
  transport/
    BleTransport.cpp/.h    # GATT, security, pairing state, RX/TX protocol
    BleSecurity.cpp/.h     # Bonds, pairing window, secure access policy
  extensions/
    WanderScreens.cpp/.h
    Attention.cpp/.h
    SecurityEvents.cpp/.h
    Quota.cpp/.h
  utils/
    Config.h
    JsonProtocol.cpp/.h
    Backlight.cpp/.h
  main.cpp
```

### macOS Tauri Bridge

```text
desk-pet-bridge/
  src/                         # Minimal TS/HTML tray status UI
  src-tauri/
    Cargo.toml
    tauri.conf.json
    capabilities/default.json
    icons/trayTemplate.png
    src/
      main.rs                  # Tauri lifecycle and command registration
      tray.rs                  # Menu / connection / pairing indicator
      ble/
        mod.rs
        client.rs              # Scan/connect/discover/write
        reconnect.rs           # Backoff and keepalive
        pairing.rs             # Intentional pairing flow and bond management
      bridge/
        http.rs                # 127.0.0.1:8765 endpoints
        protocol.rs            # Event structs and validation
        queue.rs               # Buffer during reconnect
      cursor/events.rs         # Cursor hook event mapping
      state.rs
      diagnostics.rs           # Redacted local logs
    Info.plist
  scripts/cursor-hook.sh
  README.md
```

### Event Flow

```text
Cursor native hook
  → scripts/cursor-hook.sh
  → POST http://127.0.0.1:8765/v1/cursor/event
  → Tauri validates/maps/queues event
  → authenticated + encrypted BLE Command RX write
  → ESP32 Mochi-derived state machine
  → face / screen / attention animation
```

The shell script is intentionally thin: read hook JSON from stdin, normalize non-sensitive fields, POST to `127.0.0.1`, then exit promptly.

### Local HTTP API

Bind only to `127.0.0.1:8765`; never to `0.0.0.0`.

- `POST /v1/cursor/event`: accept normalized hook data and queue a BLE event.
- `POST /v1/test`: manually test state(s).
- `GET /v1/status`: bridge/BLE state, queue size, timestamp; no sensitive payloads.
- `POST /v1/power`: send `sleep`/`wake`.
- `POST /v1/pairing/start`: explicit request to open a 120-second pairing window.
- `POST /v1/pairing/stop`: close pairing immediately.

---

## Cursor Hooks Integration

Cursor supports local subprocess hooks defined at project or user scope. A `.sh` script is the preferred integration point: it normalizes Cursor hook input and posts it to the loopback Tauri bridge.

Candidate events:
- `sessionStart` / `sessionEnd`
- `stop`
- `afterAgentResponse`
- `afterAgentThought`
- `beforeShellExecution` / `afterShellExecution`
- `beforeMCPExecution` / `afterMCPExecution`
- `beforeReadFile` / `afterFileEdit`
- `beforeSubmitPrompt`
- `preToolUse` / `postToolUse` / `postToolUseFailure`
- `subagentStart` / `subagentStop`

### Initial Mapping

| Cursor hook/event | Desk pet event | Display behavior |
|---|---|---|
| `sessionStart` | `sessionStart`, `coding` | Enter agent mode; focused expression |
| `afterAgentThought` | `stateUpdate`, `thinking` | Thinking expression |
| `beforeShellExecution` | `stateUpdate`, `coding` | Focused/coding expression |
| `afterAgentResponse` | `stateUpdate`, `waiting_for_user` when applicable | Expectant expression; start attention timer only for input/approval |
| `postToolUseFailure` | `stateUpdate`, `error` | Red/error expression |
| `sessionEnd` | `sessionEnd` | Resume wander mode |

Do not assume every event fires in every Cursor context; fall back gracefully and log only redacted diagnostic metadata.

### Security Events

Cursor hooks may not reliably identify all SSH/GitHub login UI prompts. Add optional local bridge event sources:
- Explicit security endpoint callable by user-approved local scripts.
- Opt-in shell-command heuristics for known `ssh`, `gh auth login`, or deployment commands.
- Never send credentials, tokens, full command arguments, or sensitive prompt text to ESP32 or diagnostics.

---

## Development Phases

### Phase 1: Port Mochi Base to PlatformIO
- Fork/copy original Mochi firmware.
- Preserve verified display init/pin mapping/backlight/face rendering.
- Compile/upload via PlatformIO in Cursor.
- Keep Arduino IDE 2 recovery sketch documented.

### Phase 2: BLE Transport and Security
- Implement specified GATT UUIDs.
- Add encrypted/authenticated RX/TX characteristics.
- Add bonds in NVS, pairing-window controls, and bond erase recovery.
- Test trusted reconnect plus rejected unauthorized access.

### Phase 3: Tauri Menu-Bar Bridge
- Build tray-only Tauri app with Bluetooth permission text.
- Implement BLE discovery/connect/write/reconnect using `btleplug`.
- Implement pairing controls and `127.0.0.1:8765` API.

### Phase 4: Cursor Hook Scripts
- Add `cursor-hook.sh` and hook config.
- Map key lifecycle events to compact BLE messages.
- Test end-to-end Cursor → bridge → encrypted BLE → ESP32.

### Phase 5: Wander Screens and Quota
- Add original-Mochi-style idle animation.
- Add Mac-provided weather/market/quote updates.
- Add quota bar.

### Phase 6: Attention & Security
- Add timer escalation and security overlays.
- Verify alerts are clear without leaking sensitive content.

### Phase 7: Power Polish
- Add Mac sleep/wake events.
- Dim/off backlight based on Mac state and schedule.
- Consider deep sleep only after BLE wake/reconnect behavior is verified.

---

## Acceptance Criteria

- [ ] Base firmware is derived from original Clawd Mochi code.
- [ ] Device advertises generic `DP-<last4-MAC>` name and custom BLE service.
- [ ] Unbonded/unencrypted centrals cannot write commands, configure the device, or subscribe to protected status notifications.
- [ ] Pairing is disabled by default after provisioning and opens only through explicit trusted/physical/USB action for at most 120 seconds.
- [ ] Bond data persists across ESP32 reboot; trusted Mac reconnects automatically.
- [ ] Tauri bridge discovers, connects, and reconnects automatically.
- [ ] Tauri local API binds only to loopback.
- [ ] Cursor hook updates display within 1 second when BLE is connected.
- [ ] Agent mode has original-Mochi-inspired graphics per state.
- [ ] Wander mode runs while BLE is idle/disconnected.
- [ ] Attention escalation works.
- [ ] Security alerts are visually distinct and avoid sensitive data.
- [ ] Explicit Mac sleep event dims/off backlight.
- [ ] Arduino IDE 2 can still install a recovery/display test firmware.

---

## Scope and Memory Notes

- ESP32-C3 with 4 MB flash and ~320 KB RAM should fit original Mochi-style graphics, BLE, compact JSON, state machine, and several simple screens if large raster image assets are avoided.
- Keep ESP32 BLE-first; do not include Wi-Fi + TLS/API fetching unless needed later.
- Avoid full-screen redraw loops; use dirty-region/incremental rendering.
- Keep BLE payloads compact and below negotiated ATT MTU initially.

---

## Appendix: Original Clawd Mochi Assets

Original project assets:
- Reference firmware: `clawd_mochi.ino` with verified display wiring and SPI initialization.
- 3D models: `clawd_mochi_v1.stl`, `clawd_mochi_v1.3mf`.
- Graphics: avatar, banners, and eye-expression imagery in `pics/`.

Use these as the base/inspiration for the final firmware and enclosure graphics.
