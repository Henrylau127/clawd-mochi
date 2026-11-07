#!/usr/bin/env bash
# Codex / OMP → desk pet bridge (loopback only).
# Usage: agent-hook.sh <codex|omp> <hookName> [outcome]
#
# Codex writes one JSON object to stdin. That object can include prompts,
# paths, transcripts, tool commands, and error text. This script discards it.
# For PostModelRequest it keeps only status when that value is completed,
# failed, or cancelled. OMP should pass an allowlisted outcome as $3 and
# should not put event data on stdin.
# Posted JSON is hook + source + optional outcome + timestamp.
set -euo pipefail

PROVIDER="${1:-}"
HOOK_NAME="${2:-}"
OUTCOME="${3:-}"

case "$PROVIDER" in
  codex|omp) ;;
  *) exit 0 ;;
esac

if [ -t 0 ]; then
  :
elif [ "$PROVIDER" = "codex" ] && [ "$HOOK_NAME" = "PostModelRequest" ] && [ -z "$OUTCOME" ] && command -v jq >/dev/null 2>&1; then
  OUTCOME="$(jq -r 'if .status == "completed" or .status == "failed" or .status == "cancelled" then .status else empty end' 2>/dev/null || true)"
elif [ -z "$HOOK_NAME" ] && command -v jq >/dev/null 2>&1; then
  parsed="$(jq -r '[
    (if (.hook_event_name | type) == "string" and (.hook_event_name | test("^[A-Za-z_][A-Za-z0-9_]*$")) then .hook_event_name else "" end),
    (if .status == "completed" or .status == "failed" or .status == "cancelled" then .status else "" end)
  ] | @tsv' 2>/dev/null || true)"
  HOOK_NAME="${parsed%%$'\t'*}"
  rest="${parsed#*$'\t'}"
  if [ "$rest" != "$parsed" ] && [ -z "$OUTCOME" ]; then
    OUTCOME="$rest"
  fi
else
  cat >/dev/null || true
fi

case "$OUTCOME" in
  ""|completed|failed|cancelled|error|ok|continue|done) ;;
  *) OUTCOME="" ;;
esac

case "$PROVIDER:$HOOK_NAME" in
  codex:SessionStart|codex:SessionEnd|codex:UserPromptSubmit|codex:PreModelRequest|codex:PostModelRequest|codex:PreToolUse|codex:PostToolUse|codex:PermissionRequest|codex:Stop|codex:Interrupt|codex:SubagentStart|codex:SubagentStop|codex:PreCompact|codex:PostCompact) ;;
  omp:session_start|omp:session_switch|omp:session_shutdown|omp:agent_start|omp:before_agent_start|omp:turn_start|omp:tool_call|omp:tool_execution_start|omp:tool_execution_end|omp:agent_end|omp:session_stop) ;;
  *) exit 0 ;;
esac

if [ -n "${DESKPET_BRIDGE_URL:-}" ]; then
  BRIDGE_URL="$DESKPET_BRIDGE_URL"
else
  PORT="${DESKPET_BRIDGE_PORT:-}"
  SIDECAR="${HOME:-}/.deskpet-bridge.json"
  if [ -z "$PORT" ] && [ -f "$SIDECAR" ] && command -v jq >/dev/null 2>&1; then
    PORT="$(jq -r '.port // empty' "$SIDECAR" 2>/dev/null || true)"
  fi
  PORT="${PORT:-8765}"
  BRIDGE_URL="http://127.0.0.1:${PORT}/v1/${PROVIDER}/event"
fi

TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [ -n "$OUTCOME" ]; then
  PAYLOAD="$(printf '{"hook":"%s","source":"%s","outcome":"%s","ts":"%s"}' \
    "$HOOK_NAME" "$PROVIDER" "$OUTCOME" "$TS")"
else
  PAYLOAD="$(printf '{"hook":"%s","source":"%s","ts":"%s"}' \
    "$HOOK_NAME" "$PROVIDER" "$TS")"
fi

if [ "${DESKPET_HOOK_DRY_RUN:-}" = "1" ]; then
  printf '%s\n' "$PAYLOAD"
  exit 0
fi

if command -v curl >/dev/null 2>&1; then
  curl -sf -X POST "$BRIDGE_URL" \
    -H "Content-Type: application/json" \
    -d "$PAYLOAD" \
    --max-time 2 \
    >/dev/null 2>&1 || true
fi

exit 0
