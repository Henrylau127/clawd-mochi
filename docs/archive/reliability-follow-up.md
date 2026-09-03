# Desk Pet Reliability Follow-up

Updated: 2026-09-04

## Completed

### Firmware

- BLE command, configuration, and status access now requires authenticated encryption.
- New centrals are rejected unless the device-side 120-second pairing window is open through USB `PAIR` or the GPIO9 long press.
- Bonded reconnects remain allowed.
- The passkey is stable for one pairing window and the TFT displays it when authentication begins.
- Secure-session status is initialized and refreshed for the bridge handshake.
- State transitions validate inputs before mutating state.
- Quota values are bounded safely.
- Attention timers reset across sessions without overwriting the underlying agent expression.
- Security overlays restore the underlying scene after expiry.
- Wander mode, wake/reconnect behavior, PWM backlight dimming, and high-frequency dirty-region rendering were corrected.
- Firmware version is `0.2.2-mac2`.
- **2026-09-04 pairing-race fix:** keep the BLE link up during an open pairing window (do not disconnect on insecure GATT/auth-fail mid-pairing), call `startSecurity` on connect, expose base READ/WRITE with ENC/AUTHEN, and stop wander from overpainting the passkey screen. STATUS JSON buffer enlarged so `secure_session` + `fw` no longer truncate.

### macOS bridge

- The bridge performs a protected status read before subscribing to Status TX or reporting the session ready.
- Status subscription and secure-session confirmation are required.
- Invalid status notifications terminate the session instead of being logged only.
- Automatic reconnect uses the persisted active pet with a cached-identifier fast path and scan fallback.
- First-time enrollment now requires the device-side pairing action.
- Public HTTP status no longer exposes queued event contents or `details.message`.
- Event messages are removed before BLE transmission and diagnostics retain only safe security labels.
- macOS bond removal is documented as a manual System Settings step because the production `btleplug` API does not provide unpairing.
- **2026-09-04 reconnect UX:** “not found” retries use a short fixed delay (not error backoff); known-pet scans are ~1.5s; error backoff starts at 500ms and caps at 8s.

### Repository and validation

- Earlier remediation commit on `main`: `f52cf24e7baa4f89a6e83ab01e6160ca4311f2a9`.
- Firmware native protocol tests: 18 passed.
- ESP32-C3 regular and debug builds: passed.
- Bridge Rust tests: 54 passed.
- `pio check -e native`: clean after suppressing known ArduinoJson/`unusedFunction` noise.
- Hardware E2E on this Mac (2026-09-04): Add pet bond for `DP-CC98`, encrypted `/v1/test`, Cursor hook traffic, bridge restart reconnect without PIN. See `docs/mac-bridge-status.md`.

## Hardware results (2026-09-04)

Expected serial after successful Add pet:

```text
BLE connected encrypted=no authenticated=no bonded=no
BLE auth complete encrypted=yes authenticated=yes bonded=yes
```

Bridge after secure session:

```bash
curl -s http://127.0.0.1:6900/v1/status
# ble=connected secure_session=ready device=DP-CC98 enrollment=success

curl -s -X POST http://127.0.0.1:6900/v1/test \
  -H 'Content-Type: application/json' \
  -d '{"state":"coding"}'
# {"ok":true,"queued":false}
```

## Remaining

1. Tier 4 human TFT checks (expressions, attention, security overlay, dimming).
2. Optional phone nRF Connect (Mac BLE MCP + Add pet already cover Tier 3 intent).
3. Mac sleep/wake backlight path if/when enabled in the bridge.
4. Commit/push the pairing-race + reconnect + docs updates when ready.
5. Optional: configure PlatformIO include paths for IDE clang diagnostics.
