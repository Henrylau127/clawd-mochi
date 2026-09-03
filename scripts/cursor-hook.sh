#!/usr/bin/env bash
# Cursor hook → desk pet bridge (requires Tauri menu-bar app on loopback).
# Usage: cursor-hook.sh <hookName>   (stdin = Cursor hook JSON; not forwarded)
set -euo pipefail

HOOK_NAME="${1:-unknown}"

if [ -z "${DESKPET_BRIDGE_URL:-}" ]; then
  PORT="${DESKPET_BRIDGE_PORT:-}"
  SIDECAR="${HOME}/.deskpet-bridge.json"
  if [ -z "$PORT" ] && [ -f "$SIDECAR" ] && command -v jq >/dev/null 2>&1; then
    PORT="$(jq -r '.port // empty' "$SIDECAR" 2>/dev/null || true)"
  fi
  PORT="${PORT:-8765}"
  BRIDGE_URL="http://127.0.0.1:${PORT}/v1/cursor/event"
else
  BRIDGE_URL="$DESKPET_BRIDGE_URL"
fi

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
