#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
BENCH_NAME=${BENCH_NAME:-context_borrow}
METRIC_NAME=${METRIC_NAME:-borrow_hot_path}
RESULT_PATH=${RESULT_PATH:-target/criterion/${BENCH_NAME}/${METRIC_NAME}/new/estimates.json}
THRESHOLD_MS=${THRESHOLD_MS:-100.0}
TIME_FACTOR=${TIME_FACTOR:-1.0}
RUN_BENCH=${RUN_BENCH:-0}

pushd "$PROJECT_ROOT" >/dev/null

if [[ "${RUN_BENCH}" == "1" || ! -f "$RESULT_PATH" ]]; then
  cargo bench -p nexus-actor-bench --bench reentrancy >/dev/null
fi

if [[ ! -f "$RESULT_PATH" ]]; then
  echo "Benchmark result file not found: $RESULT_PATH" >&2
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
scaled_threshold_ms=$(awk -v threshold="$THRESHOLD_MS" -v factor="$TIME_FACTOR" 'BEGIN {
  if (factor <= 0) factor = 1.0;
  printf "%.6f", threshold * factor;
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
