#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "Usage: $(basename "$0") <prompt>" >&2
  exit 1
fi

sanitize() {
  local input="$1"
  input="${input//\`/'}"
  input="${input//\"/'}"
  printf '%s' "$input"
}

RAW_PROMPT="$*"
PROMPT="$(sanitize "$RAW_PROMPT")"

CLAUDE_BIN=${CLAUDE_BIN:-claude}

"$CLAUDE_BIN" --print -p "$PROMPT" \
  --append-system-prompt "あなたはドキュメントを管理できる開発者です" \
  --allowedTools "Edit,Read,Write" \
  --permission-mode acceptEdits
