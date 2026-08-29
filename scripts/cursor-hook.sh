#!/usr/bin/env bash
# Cursor hook → desk pet bridge (requires Tauri menu-bar app on 127.0.0.1:8765).
# Usage: cursor-hook.sh <hookName>   (stdin = Cursor hook JSON; not forwarded)
set -euo pipefail

HOOK_NAME="${1:-unknown}"
BRIDGE_URL="${DESKPET_BRIDGE_URL:-http://127.0.0.1:8765/v1/cursor/event}"

case "$HOOK_NAME" in
  sessionStart)       EVENT="sessionStart"; STATE="coding" ;;
  sessionEnd)         EVENT="sessionEnd";   STATE="" ;;
  afterAgentThought)  EVENT="stateUpdate";  STATE="thinking" ;;
  beforeShellExecution|beforeMCPExecution|preToolUse)
                      EVENT="stateUpdate";  STATE="coding" ;;
  afterAgentResponse) EVENT="stateUpdate";  STATE="waiting_for_user" ;;
  afterShellExecution|postToolUse)
                      EVENT="stateUpdate";  STATE="idle" ;;
  postToolUseFailure) EVENT="stateUpdate";  STATE="error" ;;
  *)                  EVENT="stateUpdate";  STATE="idle" ;;
esac

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

PAYLOAD=$(jq -n \
  --arg event "$EVENT" \
  --arg state "$STATE" \
  --arg hook "$HOOK_NAME" \
  --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  '{
    event: $event,
    state: (if $state == "" then null else $state end),
    source: "cursor",
    hook: $hook,
    ts: $ts
  }')

if command -v curl >/dev/null 2>&1; then
  curl -sf -X POST "$BRIDGE_URL" \
    -H "Content-Type: application/json" \
    -d "$PAYLOAD" \
    --max-time 2 \
    >/dev/null 2>&1 || true
fi

exit 0
