#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "$0")/.." && pwd)"

cd "$workspace_root"

cargo test -p nexus-utils-embedded-rs --no-default-features --features embassy "$@"
cargo test -p nexus-actor-embedded-rs --no-default-features --features embassy --lib --no-run "$@"
