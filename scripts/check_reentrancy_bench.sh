#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
BENCH_TARGET="target/criterion/reentrancy/load/new/estimates.json"
TIME_FACTOR=${TIME_FACTOR:-1.0}
BASE_THRESHOLD_NS=25000000

pushd "$PROJECT_ROOT" >/dev/null

if [ "${SKIP_BENCH:-0}" != "1" ]; then
  cargo bench -p nexus-actor-core-rs --bench reentrancy >/dev/null
fi

if [ ! -f "$BENCH_TARGET" ]; then
  BENCH_TARGET=$(find target/criterion/reentrancy -name estimates.json 2>/dev/null | head -n 1 || true)
fi

if [ -z "$BENCH_TARGET" ] || [ ! -f "$BENCH_TARGET" ]; then
  echo "Bench results not found under target/criterion/reentrancy" >&2
  exit 1
fi

MEAN_NS=$(jq '.mean.point_estimate' "$BENCH_TARGET")
MEDIAN_NS=$(jq '.median.point_estimate' "$BENCH_TARGET")
STDDEV_NS=$(jq '.std_dev.point_estimate' "$BENCH_TARGET")

SCALED_THRESHOLD_NS=$(awk -v base="$BASE_THRESHOLD_NS" -v factor="$TIME_FACTOR" 'BEGIN {
  if (factor <= 0) factor = 1.0;
  printf "%.0f", base * factor;
}')

cat <<REPORT
reentrancy/load
  mean   : $(printf "%.3f ms" "$(bc -l <<< "$MEAN_NS/1000000")")
  median : $(printf "%.3f ms" "$(bc -l <<< "$MEDIAN_NS/1000000")")
  stddev : $(printf "%.3f ms" "$(bc -l <<< "$STDDEV_NS/1000000")")
  limit  : $(printf "%.3f ms" "$(bc -l <<< "$SCALED_THRESHOLD_NS/1000000")")
REPORT

if (( $(printf '%.0f\n' "$MEAN_NS") > SCALED_THRESHOLD_NS )); then
  echo "Mean execution time exceeded threshold" >&2
  exit 2
fi

popd >/dev/null
