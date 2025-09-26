#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "Usage: $(basename "$0") <prompt>" >&2
  exit 1
fi

sanitize() {
  python3 - "$1" <<'PY'
import sys
text = sys.argv[1]
text = text.replace('`', "'").replace('"', "'")
print(text, end='')
PY
}

RAW_PROMPT="$*"
PROMPT="$(sanitize "$RAW_PROMPT")"

CLAUDE_BIN=${CLAUDE_BIN:-claude}

"$CLAUDE_BIN" --print -p "$PROMPT" \
  --append-system-prompt "あなたはドキュメントを管理できる開発者です" \
  --allowedTools "Edit,Read,Write" \
  --permission-mode acceptEdits
