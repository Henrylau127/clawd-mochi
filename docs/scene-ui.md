# Scene ticker and celebrate face

Claudinho-style cues on the existing Mochi face. Transport stays encrypted BLE. Cursor hooks are unchanged; the bridge adds short labels from the hook name already in the payload.

## What shows up

| Moment | Face | Caption |
|---|---|---|
| Session starts | Coding eyes, small smile | waking up |
| Thought | Squint | thinking |
| Tool, shell, MCP | Coding eyes | working, reading, editing, or using a tool |
| Reply finished | Waiting eyes | waiting for you |
| Tool failure | Error eyes | something failed |
| `git commit` or a test command finishes in a payload that includes `scene` or `command` | Smile and sparkles | committed or tests passed |
| Wander idle | Looking / blinking | wandering |
| Quota | Corner ring, same green / yellow / red thresholds | — |

Optional `mood` is `happy`, `worried`, or `startled`. It only adds a mouth or brow mark. The bridge does not infer mood from prompt text.

`caption` and `mood` are optional. Older payloads without them still drive coding, thinking, waiting, and error. Firmware fills a default caption from the expression when the field is absent.

Labels are an allowlist. Paths, shell text, and secret-shaped tokens are dropped and never drawn. A `command` field is classified on the bridge and removed before the BLE write. The Cursor hook script still does not send the command.

## Try it

Serial monitor, 115200:

```text
TEST thinking
TEST celebrate
TEST error
WANDER
STATUS
```

`TEST celebrate` shows the sparkle face and the caption `nice work`. `STATUS` includes `caption` when one is set.

Loopback, after the bridge is running (port is often not 8765; read `loopback_addr` from status):

```bash
curl -s -X POST http://127.0.0.1:8765/v1/test \
  -H 'Content-Type: application/json' \
  -d '{"state":"celebrate"}'

curl -s -X POST http://127.0.0.1:8765/v1/cursor/event \
  -H 'Content-Type: application/json' \
  -d '{"hook":"afterAgentThought"}'

curl -s -X POST http://127.0.0.1:8765/v1/cursor/event \
  -H 'Content-Type: application/json' \
  -d '{"hook":"afterShellExecution","scene":"commit"}'
```

`scene` may be `commit`, `test`, `commit_failed`, or `test_failed`. That is the field a hook script can add later. Do not put paths or commands in `caption`.
