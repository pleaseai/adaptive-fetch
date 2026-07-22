#!/usr/bin/env bash
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

INPUT=$(cat) || exit 0

TOOL=$(printf '%s' "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null) || exit 0
if [ "$TOOL" != "WebFetch" ]; then
  exit 0
fi

URL=$(printf '%s' "$INPUT" | jq -r '.tool_input.url // empty' 2>/dev/null) || exit 0
if [ -z "$URL" ]; then
  exit 0
fi

BIN="${CLAUDE_PLUGIN_ROOT:-}/skills/adaptive-fetch/engine/bin/adaptive-fetch"
if [ ! -x "$BIN" ]; then
  BIN="$(command -v adaptive-fetch 2>/dev/null || true)"
fi
if [ -z "$BIN" ] || [ ! -x "$BIN" ]; then
  exit 0
fi

PRESETS="${CLAUDE_PLUGIN_ROOT:-}/skills/adaptive-fetch/url_presets.toml"
# check-url uses exit 10 for a match; preserve that output while failing open for
# every other non-zero status.
OUT=""
if OUT=$("$BIN" check-url "$URL" --presets "$PRESETS" --json 2>/dev/null); then
  STATUS=0
else
  STATUS=$?
fi
if [ "$STATUS" -ne 0 ] && [ "$STATUS" -ne 10 ]; then
  exit 0
fi

MATCHED=$(printf '%s' "$OUT" | jq -r '.matched // false' 2>/dev/null) || exit 0
if [ "$MATCHED" != "true" ]; then
  exit 0
fi

# Fail open until the engine can actually service the fetch. In the M0 scaffold
# adaptive-fetch returns "unimplemented", so denying WebFetch here would strand the
# request with no working alternative. `engine_ready` flips to true when M1 lands.
READY=$(printf '%s' "$OUT" | jq -r '.engine_ready // false' 2>/dev/null) || exit 0
if [ "$READY" != "true" ]; then
  exit 0
fi

REASON=$(printf '%s' "$OUT" | jq -r '.reason // empty' 2>/dev/null) || exit 0
CMD=$(printf '%s' "$OUT" | jq -r '.suggested_command // empty' 2>/dev/null) || exit 0
if [ -z "$CMD" ]; then
  exit 0
fi

MSG="adaptive-fetch: this host is preset to bypass the built-in WebFetch."
if [ -n "$REASON" ]; then
  MSG="${MSG} ${REASON}"
fi
MSG="${MSG} Run this instead:

  ${CMD}
Use the adaptive-fetch skill; do not retry WebFetch for this host."

jq -n --arg r "$MSG" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:$r}}' || exit 0
exit 0
