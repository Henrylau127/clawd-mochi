# Mac bridge session status (2026-09-04)

Wrap-up of the Mac-host work after the WSL firmware pass. Firmware Tiers 0–2 remain in [wsl-validation.md](wsl-validation.md). How to run tools on this Mac is in [mac-validation.md](mac-validation.md).

## Add pet does nothing until a pet is advertising

Yes. That is expected with no ESP32 Super Mini powered and advertising.

**Add pet** does not invent a roster entry or authorize a new bond remotely. It asks the user to
open the device-side window with serial `PAIR` or GPIO9, disconnects any current session, and scans
for an unknown `DP-*` device. If nothing is broadcasting, scan returns empty, the dropdown stays
**No pet paired**, and the roster is unchanged.

What you should see:

- The **Pairing window** toggle in Settings turns on (status poll is about 1s).
- Home badge may briefly show **SCANNING**, then **OFFLINE** again.

To actually enroll a pet:

1. Flash and power the ESP32.
2. Start the device-side pairing window with serial `PAIR` or a GPIO9 long-press so the TFT shows a passkey.
3. Click **Add pet** (or use the pairing guidance in Settings).
4. Accept the macOS Bluetooth / passkey prompt.
5. The dropdown should show a real name such as `DP-A1B2`.

Without hardware, use the mock instead:

```bash
cd desk-pet-bridge
DESKPET_BLE_MOCK=1 pnpm tauri dev
```

In mock mode, **Add pet** enrolls `DP-TEST` and connects. That path does not exercise CoreBluetooth.

## Done

### Firmware (already on WSL, confirmed on this Mac)

- PIO firmware tree, JSON protocol, non-blocking loop, serial diagnostics.
- BLE GATT + pairing window + NVS bonds in `src/transport` (serial PAIR/UNBOND validated on WSL Tier 2;
  the Mac bridge now forces a secure GATT handshake before reporting connected).
- Wander / quota / attention / security / backlight in `src/extensions`.
- Host `test_protocol` and `./scripts/validate-tier0.sh`.
- Firmware version `0.2.2-mac2` in `src/utils/Config.h`.
- Linear: [HEN-9](https://linear.app/henrylau127/issue/HEN-9), [HEN-10](https://linear.app/henrylau127/issue/HEN-10), [HEN-6](https://linear.app/henrylau127/issue/HEN-6) marked Done.

### Mac environment

- `mise.toml` puts `pio` and `cargo` on PATH.
- `scripts/check-mac-env.sh` and `scripts/validate-host.sh`.
- PlatformIO native platform, Rust, `tauri-cli`, pnpm.

### Tauri menu-bar bridge (`desk-pet-bridge/`) — [HEN-11](https://linear.app/henrylau127/issue/HEN-11)

- Menu-bar popover (not a separate window). Native Liquid Glass via `window-vibrancy`.
- Loopback HTTP for Cursor hooks. Port range 1–65535, occupancy check before rebind, sticky Apply errors, Cancel (red X) / Apply (green tick), Escape reverts the field. Sidecar `~/.deskpet-bridge.json`.
- Port-change listener no longer tight-loops (`borrow_and_update` on the tokio watch). Last local port used: **6900**.
- Mock BLE (`DESKPET_BLE_MOCK=1`, in-process `DP-TEST`). `cargo test --lib`: 49+ passed (roster, port stability, secure-session, privacy, and mock API coverage).
- Queue tile opens a live sheet; titles are human copy (e.g. **Bot is thinking**).
- Settings sheet: provider (Cursor only; Codex / Claude Code / Pi disabled), pets, pairing, loopback, test pet.
- Settings grows the popover; close / Escape restores home height.
- Pets row: **Active pet** + dropdown, then **Add pet** (blue +) and **Remove pet** (red trash). One active connection. Roster in the sidecar. Device-confirmed pairing enrolls unknown devices; reconnect stays on the active pet.
- Unpaired copy is **No pet paired** / **Pair a pet in Settings**, not `DP-xxxx`.
- Original 16×16 cube-pet icon (blue `#3b82f6`), not Anthropic’s Clawd crab. Tray uses a template glyph.

### Cursor hooks — [HEN-7](https://linear.app/henrylau127/issue/HEN-7)

- `scripts/cursor-hook.sh` and `.cursor/hooks.json` POST to the loopback API.
- Provider persisted in the sidecar; only `cursor` is accepted.

### CI

- `.github/workflows/build.yml` uploads two artifacts: ESP32 firmware package and a universal macOS DMG.

## Hardware validation log (2026-09-04)

Board: ESP32-C3 Super Mini `30:ED:A0:65:CC:98` → `DP-CC98` on `/dev/cu.usbmodem3101`. Bridge loopback `127.0.0.1:6900`.

| Check | Result | Evidence |
|-------|--------|----------|
| Reflash current tree | PASS | `pio run -t upload -e esp32-c3-super-mini` (STATUS buffer + pairing-race fixes) |
| Serial STATUS/TEST/PAIR/UNBOND/SLEEP/WAKE | PASS | serial MCP; STATUS includes `secure_session` + complete `fw` |
| Independent BLE reject (pairing closed) | PASS | BLE MCP connect → firmware `BLE rejected: pairing window closed` |
| Independent BLE + Add pet bond | PASS | After firmware fix: `BLE auth complete encrypted=yes authenticated=yes bonded=yes`; bridge `enrollment:success`, `secure_session:ready` |
| `POST /v1/test` coding | PASS | `{"ok":true,"queued":false}`; status `pet_mode:agent` / `pet_state:coding` |
| Bridge restart reconnect (no PIN) | PASS | Auto-reconnect to `DP-CC98` without passkey; early attempts were slow (scan+backoff) so **Reconnect** felt required — reconnect path tightened afterward |
| Cursor hooks while bonded | PASS | Live Cursor session drove continuous encrypted Command RX writes; bridge `last_event` updates |

### Pairing-race fix (required for Mac passkey UX)

Before the fix, **Add pet** connected then firmware disconnected on insecure GATT access, so macOS never showed a passkey dialog and wander overpainted the TFT code. Fixed in firmware:

- Keep the link up while the pairing window is open.
- Start LE security on connect during pairing.
- Add base `READ`/`WRITE` properties with ENC/AUTHEN.
- Do not let wander redraw over the pairing screen.

### Reconnect UX note

Auto-reconnect is intended. After bridge restart, “not found” retries now use a short fixed delay (not error backoff), known-pet scans are ~1.5s, and error backoff caps at 8s. **Reconnect** still resets backoff for an immediate try.

## Still needed

1. **Tier 4 display** — human confirmation that TFT expressions/colors match agent state (attention timings, security overlay, dimming).
2. **Other hook providers** — Codex, Claude Code, Pi stay disabled until those adapters exist.
3. **Mac sleep/wake → backlight** — only if/when the bridge exposes that path.
4. **Git checkpoint** — commit/PR when you want the validation + pairing-race fix saved.

Phone nRF Connect is optional now that Mac Tier 3-equivalent checks (independent BLE MCP + Add pet) passed on this host.

## How to pick up

```bash
./scripts/check-mac-env.sh
./scripts/validate-host.sh
cd desk-pet-bridge && pnpm tauri dev
```

With hardware already bonded: bridge should reconnect to the active pet automatically. Checklist is in [mac-validation.md](mac-validation.md).
