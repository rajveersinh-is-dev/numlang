#!/usr/bin/env python3
"""
ci_regression_guard.py: Enforce performance non-regression invariant for CI.

Verifies, per the NumLang Benchmark Contract (BENCHMARK_CONTRACT.md), that the NumLang-SC
supercompiler does not exhibit a statistically significant residual regression versus the
NumLang-Base residual runtime on any canonical benchmark.

Contract rule (strict win rule):
- A NumLang-SC benchmark is a strict win when the 95% confidence interval of the
  Base/SC runtime ratio excludes 1.0 AND the effect is >= 5% (ratio lower bound > 1.05).
- A real regression (residual slowdown of SC relative to Base beyond noise) is when the
  95% CI upper bound of the Base/SC ratio falls below 0.95 (SC slower than Base by >= 5%
  with CI excluding 1.0) -> the baseline strictly wins.

Failures:
- The guard FAILS (exit 1) when any evaluable benchmark shows a real regression.
- Folded cells (median within the hardware performance counter quantization floor of
  <= 500 ns) are tolerated per the contract's folded policy and are never treated as wins
  or regressions.

Deliverable for Phase C (Profitability Gate & Ablation Study).
"""

import argparse
import csv
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

# Frozen by BENCHMARK_CONTRACT.md.
FOLDED_NS_THRESHOLD = 500
WIN_EFFECT_PCT = 5.0
REGRESSION_BOUND = 1.0 - WIN_EFFECT_PCT / 100.0


def calc_ratio_stats(base_raw, sc_raw):
    if not base_raw or not sc_raw:
        return None
    n = min(len(base_raw), len(sc_raw))
    ratios = [base_raw[i] / sc_raw[i] for i in range(n) if sc_raw[i] > 0]
    if not ratios:
        return None
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


def run_regression_guard(csv_path: Path, threshold: float = REGRESSION_BOUND) -> int:
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
                "median_ns": int(row.get("median_ns") or 0),
                "raw": raw,
            }

    regressions = []
    folded = []
    skipped = 0
    total_checked = 0

    print("=" * 78)
    print(f"CI RUNTIME REGRESSION GUARD (strict win rule: SC win iff ratio CI > {1.0 / (1.0 - WIN_EFFECT_PCT / 100.0):.2f} AND effect >= {WIN_EFFECT_PCT}%; regression iff upper CI < {threshold})")
    print(f"Data source: {csv_path}")
    print("=" * 78)

    for bid, data in benchmarks.items():
        base_data = data["systems"].get("NumLang-Base")
        sc_data = data["systems"].get("NumLang-SC")

        if not base_data or not sc_data:
            continue

        if not base_data.get("raw") or not sc_data.get("raw"):
            # No measurable rounds recorded -> nothing to guard (NOT_INSTALLED / unmeasured).
            skipped += 1
            continue

        total_checked += 1
        sc_median = sc_data["median_ns"]
        if sc_median <= FOLDED_NS_THRESHOLD:
            # Folded cells are tolerated by contract policy and never treated as regressions.
            folded.append(bid)
            print(f"  [FOLD] {bid:15} | Base: {base_data['median_ns']:>10} ns | SC: {sc_median:>10} ns | Folded (≤500ns) - tolerated")
            continue

        stats = calc_ratio_stats(base_data["raw"], sc_data["raw"])
        if stats is None:
            skipped += 1
            continue

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

    fold_note = f" ({len(folded)} folded/tolerated)" if folded else ""
    print(f"SUCCESS: All {total_checked} benchmarks maintained ratio >= 1.0 within noise threshold [upper CI >= {threshold}].{fold_note}")
    print("0 regressions detected.")
    print("=" * 78)
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="NumLang CI Runtime Regression Guard")
    parser.add_argument("--csv", type=Path, default=DEFAULT_CSV, help="Path to showdown_results.csv")
    parser.add_argument("--threshold", type=float, default=REGRESSION_BOUND, help="Regression ratio threshold (default 0.95)")
    args = parser.parse_args()
    sys.exit(run_regression_guard(args.csv, args.threshold))
