# Mac bridge session status (2026-09-03)

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
- Mock BLE (`DESKPET_BLE_MOCK=1`, in-process `DP-TEST`). `cargo test --lib`: 49 passed (roster, port stability, secure-session, privacy, and mock API coverage).
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

## Still needed

Hardware acceptance still pending (the connected board was only queried read-only in this validation):

1. **[HEN-8](https://linear.app/henrylau127/issue/HEN-8) Mac BLE E2E (Tier 5)** — bond via **Add pet**, passkey on TFT, encrypted GATT write, reboot reconnect, `POST /v1/test` changes the face. This was not run; only serial `STATUS` was queried.
2. **[HEN-7](https://linear.app/henrylau127/issue/HEN-7) Cursor E2E** — live Cursor session while the menu-bar app is running and a bonded pet is connected.
3. **[HEN-5](https://linear.app/henrylau127/issue/HEN-5) remaining firmware BLE acceptance** — Tier 3 nRF Connect phone pairing. Mac path is HEN-8.
4. **Tier 4 display** — human check that TFT expressions match agent state.

Remaining software / documentation notes:

5. **Other hook providers** — Codex, Claude Code, Pi stay disabled until those adapters exist.
6. **serial-mcp** — optional agent serial docs, only after explicit MCP install approval.
7. **Git** — this Mac session has a large uncommitted working tree; nothing was pushed unless asked. Commit/PR when you want a checkpoint.

## How to pick up

```bash
./scripts/check-mac-env.sh
./scripts/validate-host.sh
cd desk-pet-bridge && DESKPET_BLE_MOCK=1 pnpm tauri dev
```

With hardware: flash, open pairing on the pet, then **Add pet**. Checklist is in [mac-validation.md](mac-validation.md).
