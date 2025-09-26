#!/usr/bin/env python3
"""Collect Criterion benchmark statistics and append them to a CSV log."""

from __future__ import annotations

import argparse
import csv
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, Optional, Tuple

CRITERION_ESTIMATE = "estimate.json"
CRITERION_SAMPLE = "sample.json"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--criterion-dir",
        default=Path("target") / "criterion",
        type=Path,
        help="Criterion results directory (default: target/criterion)",
    )
    parser.add_argument(
        "--output",
        default=Path("bench_history.csv"),
        type=Path,
        help="CSV file to append benchmark rows to (default: bench_history.csv)",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Overwrite the output file instead of appending",
    )
    return parser.parse_args()


def seconds_to_ns(value: Optional[float]) -> Optional[float]:
    if value is None:
        return None
    return value * 1e9


def load_estimate(path: Path) -> Optional[Dict[str, float]]:
    try:
        data = json.loads(path.read_text())
    except FileNotFoundError:
        return None
    mean = data.get("mean", {}).get("point_estimate")
    std_dev = data.get("std_dev", {}).get("point_estimate")
    median = data.get("median", {}).get("point_estimate")
    slope = data.get("slope", {}).get("point_estimate")
    return {
        "mean_ns": seconds_to_ns(mean),
        "std_dev_ns": seconds_to_ns(std_dev),
        "median_ns": seconds_to_ns(median),
        "slope_ns": seconds_to_ns(slope),
    }


def load_sample_size(path: Path) -> Optional[int]:
    if not path.exists():
        return None
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError:
        return None
    return data.get("len") or data.get("sample_count")


def walk_estimates(root: Path) -> Iterable[Tuple[Path, Dict[str, float], Optional[int]]]:
    if not root.exists():
        return
    for estimate_path in root.glob("**/new/estimate.json"):
        stats = load_estimate(estimate_path)
        if not stats:
            continue
        bench_dir = estimate_path.parents[1]  # e.g. target/criterion/group/bench
        sample_path = bench_dir / CRITERION_SAMPLE
        sample_size = load_sample_size(sample_path)
        yield estimate_path, stats, sample_size


def bench_metadata(root: Path, estimate_path: Path) -> Tuple[str, str]:
    rel_parts = estimate_path.relative_to(root).parts
    # Expected: group/bench/new/estimate.json or deeper nesting
    if len(rel_parts) < 4:
        return ("/".join(rel_parts[:-2]), "")
    group = rel_parts[0]
    bench = "/".join(rel_parts[1:-2]) or group
    return group, bench


def ensure_parent(path: Path) -> None:
    if path.parent and not path.parent.exists():
        path.parent.mkdir(parents=True, exist_ok=True)


def write_csv(
    output: Path,
    rows: Iterable[Dict[str, Optional[object]]],
    overwrite: bool = False,
) -> None:
    ensure_parent(output)
    mode = "w" if overwrite or not output.exists() else "a"
    with output.open(mode, newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(
            fh,
            fieldnames=[
                "timestamp",
                "group",
                "benchmark",
                "mean_ns",
                "std_dev_ns",
                "median_ns",
                "slope_ns",
                "sample_size",
            ],
        )
        if fh.tell() == 0:
            writer.writeheader()
        for row in rows:
            writer.writerow(row)


def main() -> None:
    args = parse_args()
    timestamp = datetime.now(timezone.utc).isoformat()
    root = args.criterion_dir

    rows = []
    for estimate_path, stats, sample_size in walk_estimates(root):
        group, bench = bench_metadata(root, estimate_path)
        row = {
            "timestamp": timestamp,
            "group": group,
            "benchmark": bench,
            "mean_ns": stats.get("mean_ns"),
            "std_dev_ns": stats.get("std_dev_ns"),
            "median_ns": stats.get("median_ns"),
            "slope_ns": stats.get("slope_ns"),
            "sample_size": sample_size,
        }
        rows.append(row)

    if not rows:
        print(f"No benchmark estimates found under {root}", file=sys.stderr)
        return

    write_csv(args.output, rows, overwrite=args.overwrite)
    print(f"Wrote {len(rows)} rows to {args.output}")


if __name__ == "__main__":
    main()
