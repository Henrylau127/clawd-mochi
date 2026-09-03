# Graphics vs original Mochi + Cursor integration

## Why the display does not look like the original repo

The desk pet firmware is **not** the original WiFi sketch. After boot (splash + logo reveal), it enters **WANDER** mode and cycles every 15 seconds:

| Screen | What you see |
|--------|----------------|
| Idle | Colored background + two vertical bars (eyes) |
| Ambient | Dark blue + randomly placed twinkling stars (new layout each 15s cycle) |

That matches `src/extensions/WanderScreens.cpp`, not a bug.

Original [clawd_mochi.ino](clawd_mochi.ino) used **orange** backgrounds, **wiggle + blink** (`animNormalEyes`), a WiFi info screen, and a phone web controller. Those are intentionally absent in BLE-first desk pet firmware (recovery sketch: `sketches/clawd_mochi_recovery/`).

Agent expressions (blue coding eyes, yellow `><` thinking, etc.) appear only in **AGENT** mode — not from wander cycling.

## Does Cursor change the graphics?

**Not yet complete end-to-end.** Cursor hooks need this chain:

```text
.cursor/hooks.json → scripts/cursor-hook.sh → Mac Tauri bridge (127.0.0.1:8765)
  → BLE → firmware → Face::drawExpression
```

The Tauri bridge lives in `desk-pet-bridge/` (Linear HEN-11). Hooks POST to loopback and stay silent until the bridge is running. See [mac-validation.md](mac-validation.md).

### Test without Cursor

```bash
# Serial (WSL)
echo STATUS | socat - /dev/ttyACM0,raw,echo=0,b115200
echo TEST thinking | socat - /dev/ttyACM0,raw,echo=0,b115200
echo '{"event":"sessionStart","state":"coding"}' | socat - /dev/ttyACM0,raw,echo=0,b115200
```

### Stuck on `><` (thinking)

AGENT `thinking` stays until another state update clears it. With the original hook set this was common:
`afterAgentThought` set thinking, but **file edits did not fire any hook**, so the face stayed on thinking through edits.

As of 2026-09-04, `.cursor/hooks.json` also maps `preToolUse` / `postToolUse` / `afterFileEdit` → coding so tool and edit activity leave thinking. Reload Cursor hooks if the sticky face persists.

```bash
echo WANDER | socat - /dev/ttyACM0,raw,echo=0,b115200
# or: echo '{"event":"sessionEnd"}' | socat - ...
```

`TEST idle` shows idle eyes but keeps AGENT mode. `WAKE` only exits SLEEP. See [wsl-validation.md](wsl-validation.md) for the full serial command table.

## Agent skill for full hook setup

See `~/.cursor/skills-cursor/desk-pet-cursor-hooks/SKILL.md` for event mapping, WSL vs Mac notes, and validation checklist.

Project templates (inactive until bridge runs):

- [.cursor/hooks.json](../.cursor/hooks.json)
- [scripts/cursor-hook.sh](../scripts/cursor-hook.sh)

## Restoring original Mochi look (firmware work)

Hooks cannot restore orange wiggle-eyes or remove ambient star screens. That requires firmware changes (port `animNormalEyes`, orange wander background, disable or lengthen wander cycle). Track as a separate feature if desired.
