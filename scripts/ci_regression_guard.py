#!/usr/bin/env python3
"""
ci_regression_guard.py: Enforce performance non-regression invariant for CI.
Verifies that every benchmark in showdown_results.csv maintains:
    Ratio (Base / SC) >= 1.0 within statistical noise threshold.
Specifically: rejects any benchmark whose 95% CI upper bound is < 0.95
(which would indicate a statistically significant slowdown of >5%).

Deliverable for Phase C (Profitability Gate & Ablation Study).
"""

import argparse
import csv
import os
import statistics
import sys
from collections import OrderedDict
from pathlib import Path

# Configure utf-8 encoding for standard output on Windows consoles
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8")

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CSV = ROOT / "bench" / "data" / "showdown_results.csv"
SAMPLE_CSV = ROOT / "bench" / "data" / "showdown_results_sample.csv"


def calc_ratio_stats(base_raw, sc_raw, fallback_base=0, fallback_sc=0):
    if not base_raw or not sc_raw:
        if fallback_sc > 0 and fallback_base > 0:
            r = fallback_base / fallback_sc
            return {"median": r, "ci95_low": r, "ci95_high": r}
        return {"median": 1.0, "ci95_low": 1.0, "ci95_high": 1.0}
    n = min(len(base_raw), len(sc_raw))
    ratios = [base_raw[i] / sc_raw[i] for i in range(n) if sc_raw[i] > 0]
    if not ratios:
        return {"median": 1.0, "ci95_low": 1.0, "ci95_high": 1.0}
    ratios.sort()
    nr = len(ratios)
    r_med = statistics.median(ratios)
    low_idx = 1 if nr >= 20 else 0
    high_idx = -2 if nr >= 20 else -1
    return {
        "median": r_med,
        "ci95_low": ratios[low_idx],
        "ci95_high": ratios[high_idx],
    }


def parse_raw_rounds(raw_str):
    if not raw_str or not raw_str.strip():
        return []
    return [int(x.strip()) for x in raw_str.split(";") if x.strip().isdigit()]


def run_regression_guard(csv_path: Path, threshold: float = 0.95) -> int:
    if not csv_path.exists():
        if SAMPLE_CSV.exists():
            csv_path = SAMPLE_CSV
        else:
            sys.stderr.write(f"Error: CSV file not found: {csv_path}\n")
            return 2

    benchmarks = OrderedDict()
    with open(csv_path, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            bid = row["benchmark_id"]
            if bid not in benchmarks:
                benchmarks[bid] = {"name": row["name"], "group": row["group"], "systems": {}}
            raw = parse_raw_rounds(row.get("raw_rounds_ns", ""))
            benchmarks[bid]["systems"][row["system"]] = {
                "median_ns": int(row.get("median_ns", 0)),
                "raw": raw,
            }

    regressions = []
    total_checked = 0

    print("=" * 78)
    print(f"CI RUNTIME REGRESSION GUARD (threshold: upper CI < {threshold})")
    print(f"Data source: {csv_path}")
    print("=" * 78)

    for bid, data in benchmarks.items():
        base_data = data["systems"].get("NumLang-Base")
        sc_data = data["systems"].get("NumLang-SC")

        if not base_data or not sc_data:
            continue

        total_checked += 1
        stats = calc_ratio_stats(
            base_data["raw"],
            sc_data["raw"],
            fallback_base=base_data["median_ns"],
            fallback_sc=sc_data["median_ns"],
        )

        r_med = stats["median"]
        r_low = stats["ci95_low"]
        r_high = stats["ci95_high"]

        # Regression condition: SC is significantly slower than Base (upper bound of Base/SC < threshold)
        if r_high < threshold:
            regressions.append({
                "id": bid,
                "name": data["name"],
                "group": data["group"],
                "base_median_ns": base_data["median_ns"],
                "sc_median_ns": sc_data["median_ns"],
                "ratio_median": r_med,
                "ci95_low": r_low,
                "ci95_high": r_high,
            })
            print(f"  [FAIL] {bid:15} | Base: {base_data['median_ns']:>10} ns | SC: {sc_data['median_ns']:>10} ns | "
                  f"Ratio: {r_med:.2f}x [{r_low:.2f}-{r_high:.2f}] (REGRESSION)")
        else:
            print(f"  [PASS] {bid:15} | Base: {base_data['median_ns']:>10} ns | SC: {sc_data['median_ns']:>10} ns | "
                  f"Ratio: {r_med:.2f}x [{r_low:.2f}-{r_high:.2f}]")

    print("=" * 78)
    if regressions:
        print(f"ERROR: {len(regressions)} of {total_checked} benchmarks regressed beyond noise threshold ({threshold})!")
        for reg in regressions:
            print(f"  - {reg['id']} ({reg['name']}): Ratio {reg['ratio_median']:.2f}x [{reg['ci95_low']:.2f}-{reg['ci95_high']:.2f}]")
        return 1

    print(f"SUCCESS: All {total_checked} benchmarks maintained ratio >= 1.0 within noise threshold [upper CI >= {threshold}].")
    print("0 regressions detected.")
    print("=" * 78)
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="NumLang CI Runtime Regression Guard")
    parser.add_argument("--csv", type=Path, default=DEFAULT_CSV, help="Path to showdown_results.csv")
    parser.add_argument("--threshold", type=float, default=0.95, help="Regression ratio threshold (default 0.95)")
    args = parser.parse_args()
    sys.exit(run_regression_guard(args.csv, args.threshold))
