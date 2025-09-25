#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
THRESHOLD_NS=${THRESHOLD_NS:-25000000}
BENCH_TARGET="target/criterion/reentrancy/load/new/estimates.json"

pushd "$PROJECT_ROOT" >/dev/null

if [ "${SKIP_BENCH:-0}" != "1" ]; then
  cargo bench -p nexus-actor-core-rs --bench reentrancy >/dev/null
fi

if [ ! -f "$BENCH_TARGET" ]; then
  echo "Bench results not found at $BENCH_TARGET" >&2
  exit 1
fi

MEAN_NS=$(jq '.mean.point_estimate' "$BENCH_TARGET")
MEDIAN_NS=$(jq '.median.point_estimate' "$BENCH_TARGET")
STDDEV_NS=$(jq '.std_dev.point_estimate' "$BENCH_TARGET")

cat <<REPORT
reentrancy/load
  mean   : $(printf "%.3f ms" "$(bc -l <<< "$MEAN_NS/1000000")")
  median : $(printf "%.3f ms" "$(bc -l <<< "$MEDIAN_NS/1000000")")
  stddev : $(printf "%.3f ms" "$(bc -l <<< "$STDDEV_NS/1000000")")
  limit  : $(printf "%.3f ms" "$(bc -l <<< "$THRESHOLD_NS/1000000")")
REPORT

if (( $(printf '%.0f
' "$MEAN_NS") > THRESHOLD_NS )); then
  echo "Mean execution time exceeded threshold" >&2
  exit 2
fi

popd >/dev/null
