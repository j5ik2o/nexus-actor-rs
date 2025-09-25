#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"

pushd "$PROJECT_ROOT" >/dev/null

rg --no-heading --with-filename --line-number "Arc<Mutex" || true

popd >/dev/null
