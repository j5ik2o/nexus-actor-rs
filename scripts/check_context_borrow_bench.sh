#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
# Criterion changed its output layout between versions, so allow override
# and fall back to the first estimates.json under the expected directory.
RESULT_PATH=${RESULT_PATH:-target/criterion/context_borrow/borrow_hot_path/new/estimates.json}
TIME_FACTOR=${TIME_FACTOR:-1.0}
BASE_THRESHOLD_MS=100.0
RUN_BENCH=${RUN_BENCH:-0}

pushd "$PROJECT_ROOT" >/dev/null

if [[ "${RUN_BENCH}" == "1" || ! -f "$RESULT_PATH" ]]; then
  cargo bench -p nexus-actor-core-rs --bench reentrancy >/dev/null
fi

if [[ ! -f "$RESULT_PATH" ]]; then
  RESULT_PATH=$(find target/criterion/context_borrow -name estimates.json 2>/dev/null | head -n 1 || true)
fi

if [[ -z "$RESULT_PATH" || ! -f "$RESULT_PATH" ]]; then
  echo "Benchmark result file not found under target/criterion/context_borrow" >&2
  exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required to parse Criterion output" >&2
  exit 3
fi

mean_ns=$(jq -r '.mean.point_estimate' "$RESULT_PATH")
std_dev_ns=$(jq -r '.standard_deviation.point_estimate' "$RESULT_PATH")

mean_ms=$(awk -v ns="$mean_ns" 'BEGIN { printf "%.6f", ns / 1e6 }')
std_dev_ms=$(awk -v ns="$std_dev_ns" 'BEGIN { printf "%.6f", ns / 1e6 }')
scaled_threshold_ms=$(awk -v base="$BASE_THRESHOLD_MS" -v factor="$TIME_FACTOR" 'BEGIN {
  if (factor <= 0) factor = 1.0;
  printf "%.6f", base * factor;
}')

printf 'context_borrow.mean_ms=%.6f\n' "$mean_ms"
printf 'context_borrow.std_dev_ms=%.6f\n' "$std_dev_ms"
printf 'context_borrow.threshold_ms=%.6f\n' "$scaled_threshold_ms"

if awk -v mean="$mean_ms" -v threshold="$scaled_threshold_ms" 'BEGIN { exit !(mean > threshold) }'; then
  echo "FAIL threshold_exceeded: mean ${mean_ms}ms > ${scaled_threshold_ms}ms" >&2
  exit 4
fi

echo "PASS threshold_ok: mean ${mean_ms}ms <= ${scaled_threshold_ms}ms"

popd >/dev/null
