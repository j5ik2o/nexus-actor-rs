#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

files=()
if [[ $# -gt 0 ]]; then
  while IFS= read -r entry; do
    files+=("$entry")
  done < <(printf '%s
' "$@" | grep '\.rs$' || true)
else
  while IFS= read -r entry; do
    files+=("$entry")
  done < <(git diff --name-only --cached --diff-filter=ACM | grep '\.rs$' || true)
  if [[ ${#files[@]} -eq 0 ]]; then
    while IFS= read -r entry; do
      files+=("$entry")
    done < <(git diff --name-only HEAD | grep '\.rs$' || true)
  fi
fi

if [[ ${#files[@]} -eq 0 ]]; then
  echo "no rust files to check"
  exit 0
fi

missing=0
printf '%s
' "${files[@]}" | python3 <<'PYTHON'
import re
import sys
from pathlib import Path

pattern = re.compile(r'^pub(?:\s+crate)?\s+(?:async\s+)?(struct|enum|trait|fn|const|type|mod|union)')
doc_comment = re.compile(r'^(///|//!|/\*\*)')
doc_attr = re.compile(r'^#\s*\[\s*doc\s*=')
allow_mod_tests = re.compile(r'^pub(?:\s+crate)?\s+mod\s+tests')
missing = []

for raw_path in sys.stdin.read().splitlines():
    if not raw_path:
        continue
    path = Path(raw_path)
    if not path.exists():
        continue
    prev_doc = False
    try:
        lines = path.read_text(encoding='utf-8').splitlines()
    except UnicodeDecodeError:
        continue
    for idx, line in enumerate(lines, 1):
        stripped = line.lstrip()
        if doc_comment.match(stripped) or doc_attr.match(stripped):
            prev_doc = True
            continue
        m = pattern.match(stripped)
        if m:
            if allow_mod_tests.match(stripped):
                prev_doc = False
                continue
            if prev_doc:
                prev_doc = False
                continue
            missing.append(f"{path}:{idx}")
        elif stripped and not stripped.startswith('#['):
            prev_doc = False

for entry in missing:
    print(f"missing rustdoc: {entry}")

if missing:
    sys.exit(1)
PYTHON

status=$?
if [[ $status -ne 0 ]]; then
  echo "rustdoc check failed" >&2
  exit $status
fi

echo "rustdoc check passed"
