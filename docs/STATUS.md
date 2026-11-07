# Desk Pet STATUS

Living checklist. Update this file in place; put dated narrative in [archive/](archive/) if needed.

**Updated:** 2026-11-07  
**Reference hardware:** ESP32-C3 Super Mini `30:ED:A0:65:CC:98` → `DP-CC98`, fw `0.2.2-mac2`  
**Bridge:** often `http://127.0.0.1:6900` (`loopback_addr` in status)

---

## Validation tiers

| Tier | Status | Notes |
|------|--------|--------|
| 0 Builds + native tests | PASS | `validate-tier0.sh` / `pio test -e native` |
| 1 Flash + boot | PASS | Mac + WSL paths |
| 2 Serial diagnostics | PASS | STATUS/TEST/PAIR/… |
| 3 BLE pair / encrypt | PASS (Mac) | BLE MCP + Add pet on `DP-CC98`; phone nRF optional |
| 4 TFT eyeball | PASS | User confirmed expression states look good |
| 5 Mac bridge + hooks | PASS | Encrypted test, hooks, restart reconnect |

Linear (Done): HEN-5, HEN-6, HEN-7, HEN-8, HEN-9, HEN-10, HEN-11.

---

## Completed (keep short)

### Firmware
- BLE-first stack, pairing window, LE SC bonding, secure session for bridge handshake.
- Pairing-race fix: keep link during pairing; no wander over passkey UI; STATUS JSON capacity 512.
- Wander / attention / quota / security / backlight extensions.

### Bridge
- Menu-bar popover, loopback API, roster, mock `DP-TEST`, secure GATT handshake.
- Auto-reconnect fixed: removed unconditional 250ms `select!` sleep that cancelled the reconnect timer.
- Stale connected UI fixed: disconnect detection outside `select!`; session ready only after first write succeeds.
- **Link verification:** UI stays VERIFYING until an encrypted firmware status read confirms `ble_connected` + `secure_session`. First check ~5s after connect; keepalive every 30s. Zombie CoreBluetooth sessions (e.g. after flash) drop within ~5s instead of showing CONNECTED.
- BLE keepalive: encrypted status read every 30s when idle (prevents CoreBluetooth idle drop).
- Tray green/yellow/red/grey dots + hover tooltips.
- Cursor hooks: tool/edit → coding (no sticky thinking).
- Codex (`.codex/hooks.json`) and oh-my-pi OMP (`.omp/hooks/post/desk-pet.js`) post the same states beside Cursor. BLE `source` is `codex` or `omp`. No caption or usage text is added.

### Firmware (reliability)
- Supervision timeout: 6s during pairing, 20s once encrypted (was 4s on connect).
- Serial logs NimBLE disconnect reason on link drop.
- Unexpected BLE drop: ends agent session, shows ~4.5s **link lost** screen, then normal wander.

### Docs
- Consolidated to [prd.md](prd.md), [architecture.md](architecture.md), [runbook.md](runbook.md), this file. Older notes under [archive/](archive/).

---

## Open

- [ ] **Git commit** of uncommitted firmware / bridge / hooks / docs work.
- [ ] **Flash fw** with longer supervision timeout + disconnect reason logging.
- [ ] **Verify keepalive** — idle bridge 5+ min, `ble: connected` stays stable.
- [ ] Tray dots on light vs dark menu bar (eyeball).
- [ ] Optional: surface `last_error` more clearly in the popover when offline.
- [ ] Optional: Mac sleep/wake → backlight if bridge exposes it.
- [ ] Optional: Claude Code and upstream Pi (`pi`) hook providers. Codex and OMP are in.
- [ ] Optional: phone nRF Connect for Tier 3 nostalgia.

---

## Pitfalls (do not regress)

1. No short unconditional sleep in the same `select!` as the BLE reconnect timer.
2. Disconnect detection must run outside hook-starved `select!` branches.
3. Do not mark the UI connected until the first queued write succeeds **and** firmware confirms the link (`link_verified`).
4. Do not “fix” hung CoreBluetooth connects with aggressive abort loops without evidence.
5. Always read `loopback_addr` from status (not hard-coded 8765).
6. Forget macOS Bluetooth bond manually after Remove pet.
7. Firmware ignores `stateUpdate` in wander mode — bridge must send `sessionStart` first (auto-injected when needed).
8. After **firmware flash**, CoreBluetooth may keep a zombie session — bridge verifies via status read; reconnect if VERIFYING persists or tray stays yellow.

---

## Quick verify

```bash
curl -s http://127.0.0.1:6900/v1/status
cd desk-pet-bridge && cargo test --manifest-path src-tauri/Cargo.toml --lib
~/.platformio/penv/bin/pio test -e native
```

Kill/restart the bridge and confirm `secure_session: ready` without clicking Reconnect.
