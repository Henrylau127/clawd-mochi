# Desk Pet session wrap-up (2026-09-04)

Follow-up notes from the Mac hardware + bridge session. Hardware: ESP32-C3 Super Mini (`DP-CC98`, MAC `30:ED:A0:65:CC:98`), firmware `0.2.2-mac2`. Bridge loopback often on `127.0.0.1:6900` (not always 8765).

**Current live status at wrap time:** `ble: connected`, `secure_session: ready`, device `DP-CC98`.

---

## What was accomplished

### Firmware (ESP32)

1. **STATUS JSON truncation** — `secure_session` made status JSON exceed 256 bytes; raised capacity to 512 and treat truncation as failure in `JsonProtocol`.
2. **Pairing-race / missing passkey UI** — during an open pairing window the firmware was disconnecting on insecure GATT/auth; wander could overpaint the passkey screen.
   - Keep link during pairing; `startSecurity` on connect; base READ/WRITE + ENC/AUTHEN.
   - Do not disconnect insecure access while pairing is open.
   - Do not draw wander over pairing UI (`WanderScreens.cpp` / `main.cpp`).
3. **Serial / BLE validation** — STATUS/TEST/PAIR/UNBOND/SLEEP/WAKE exercised; BLE reject-when-closed confirmed; Add pet bond completed (`encrypted` / `authenticated` / `bonded`).

### Mac bridge (Tauri)

1. **Tier 5 / Cursor E2E** — hooks drive expressions; `/v1/test` works when secure session ready.
2. **Sticky “thinking” during edits** — `afterAgentThought` → thinking with no exit on tools/edits.
   - Mapped `preToolUse` / `postToolUse` / `afterFileEdit` / `beforeReadFile` → **coding** in `.cursor/hooks.json`, `scripts/cursor-hook.sh`, and `cursor/events.rs`.
3. **Auto-reconnect after restart stuck offline**
   - **Root cause:** unconditional `tokio::time::sleep(250ms)` in `ble/client.rs` `select!` always won and cancelled the 500ms reconnect arm, so auto-connect never ran (only Reconnect / Add pet did).
   - **Fix:** remove that noop sleep. Verified: kill app → restart → auto-connect to `DP-CC98` without clicking Reconnect (~50s cold CoreBluetooth is slow but expected).
   - Earlier timer/timeout thrash attempts were reverted; do not reintroduce without evidence.
4. **Tray status indicator** — menu-bar icon corner dots + hover tooltip:
   - Green: connected + secure ready  
   - Yellow: scanning / securing / pairing  
   - Red: paired pet offline  
   - Grey: no pet paired  
   - Tooltips e.g. `Connected · DP-CC98`, `Securing DP-CC98…`

### Tier validation / Linear

| Item | Result |
|------|--------|
| Tier 3-equivalent (Mac BLE / pairing) | Done via bridge + BLE MCP evidence |
| Tier 4 (expression colors / eyeball) | User confirmed states look good |
| Tier 5 (Mac bridge E2E) | Done |
| HEN-5, HEN-7, HEN-8, HEN-11 | Marked Done in Linear |

Docs touched: `mac-bridge-status.md`, `mac-validation.md`, `wsl-validation.md`, `graphics-and-cursor.md`, `reliability-follow-up.md`, `desk-pet-bridge/README.md`.

---

## Key files changed (uncommitted unless noted)

### Firmware
- `src/transport/BleTransport.cpp`
- `src/extensions/WanderScreens.cpp`, `src/main.cpp`
- `src/utils/JsonProtocol.{h,cpp}`, `src/utils/SerialDiag.cpp`

### Bridge
- `desk-pet-bridge/src-tauri/src/ble/client.rs` — reconnect `select!` fix
- `desk-pet-bridge/src-tauri/src/tray.rs` — status icon + tooltip
- `desk-pet-bridge/src-tauri/icons/tray{Ready,Connecting,Offline,Idle}.png`
- `desk-pet-bridge/src-tauri/src/cursor/events.rs`
- `desk-pet-bridge/README.md`

### Cursor hooks
- `.cursor/hooks.json`
- `scripts/cursor-hook.sh`

---

## Lessons / pitfalls

1. **Reconnect dead code** — never put a shorter unconditional sleep in the same `select!` as a longer reconnect timer; the short arm cancels the long one every loop.
2. **Don’t “fix” CoreBluetooth with aggressive connect timeouts** while debugging — aborting mid-connect can leave CB wedged and look like a new bug.
3. **Loopback port** — check Settings / `GET /v1/status` (`loopback_addr`); often `6900` on this Mac.
4. **macOS bond forget** — bridge cannot unpair; System Settings → Bluetooth → Forget after Remove pet.
5. **Cold reconnect ~30–60s** is plausible; needing the Reconnect button was the real defect.

---

## Suggested follow-ups

### High value
- [ ] **Git commit** of firmware pairing fix, STATUS buffer, hooks, reconnect, tray icons, docs (not committed yet).
- [ ] **Speed up cold reconnect** (careful cached peripheral or shorter known-pet scan) without re-breaking CB — measure first.
- [ ] Confirm tray dots/tooltips on both light and dark menu bars.

### Product / UX
- [ ] Tray icon: optional animate yellow while securing; keep tooltip as source of truth.
- [ ] Surface `last_error` more clearly in the popover when red/offline.

### Firmware / hardware
- [ ] Mac sleep/wake backlight behavior (called out earlier as optional).
- [ ] Optional phone nRF Connect Tier 3 if still desired beyond Mac evidence.
- [ ] Other hook providers beyond Cursor if needed.

### Docs / process
- [ ] Refresh `docs/reliability-follow-up.md` so reconnect section matches the real fix (noop `select!` arm), not the earlier backoff-only notes.
- [ ] Keep bridge test count current (57 lib tests at wrap time including tray tests).

---

## Quick verify commands

```bash
# Bridge status
curl -s http://127.0.0.1:6900/v1/status

# Bridge unit tests
cd desk-pet-bridge && cargo test --manifest-path src-tauri/Cargo.toml --lib

# Firmware native tests
pio test -e native

# Kill + restart bridge, then poll until secure_session=ready (no Reconnect click)
```

---

## Related docs

- [mac-bridge-status.md](mac-bridge-status.md)
- [mac-validation.md](mac-validation.md)
- [wsl-validation.md](wsl-validation.md)
- [reliability-follow-up.md](reliability-follow-up.md)
- [desk-pet-firmware-plan.md](desk-pet-firmware-plan.md)
- [../desk-pet-bridge/README.md](../desk-pet-bridge/README.md)
