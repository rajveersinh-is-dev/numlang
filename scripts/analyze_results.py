#!/usr/bin/env python3
"""Independent statistical analysis of bench/data/showdown_results.csv.

Frozen contract (BENCHMARK_CONTRACT.md v1.0):
  * A cell whose median runtime is <= 500 ns is FOLDED (never a numeric win).
  * Ratio is comparator / NumLang-SC for every comparator column (>1 means SC is faster).
  * Strict SC WIN iff the 95% CI low bound of the ratio is > 1.05.
  * SC REGRESSION iff the 95% CI high bound of the ratio is < 0.95.
  * Everything else is a TIE. Folded / non-evaluable cells are excluded from win tallies.

Primary CI estimator is the one already used by scripts/make_bench_table.py and
scripts/ci_regression_guard.py: sorted per-round paired ratios with the 2nd and
(n-1)-th order statistics as bounds (n=30). This is reproduced here deliberately
so that the audit speaks the same language as the published tables. A Welch
interval on paired log-ratios is computed as an independent robustness check;
its verdicts are reported separately, never mixed into the contract counts.

Usage:
  python scripts/analyze_results.py
  python scripts/analyze_results.py --csv path/to/other.csv
  python scripts/analyze_results.py --verbose
"""

import argparse
import csv
import hashlib
import math
import statistics
import sys
from collections import Counter, OrderedDict
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8")

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CSV = ROOT / "bench" / "data" / "showdown_results.csv"

FOLDED_NS = 500          # contract section 5
REQUIRED_ROUNDS = 30     # contract section 5 (non-quick mode)
WIN_LOW = 1.05           # contract section 6
REGRESSION_HIGH = 0.95   # contract section 6
SC = "NumLang-SC"
COMPARATORS = ("NumLang-Base", "Rustc-O3", "MSVC-O2", "Clang-O3", "GCC-O3", "GHC-O2")


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest().upper()


def parse_rounds(text: str):
    """Return (rounds, malformed_tokens)."""
    if not text or not text.strip():
        return [], []
    rounds, bad = [], []
    for token in text.split(";"):
        token = token.strip()
        if not token:
            continue
        try:
            rounds.append(int(token))
        except ValueError:
            bad.append(token)
    return rounds, bad


def order_stats(values):
    """Repository estimator: median + 2nd/(n-1)-th sorted order statistics."""
    if not values:
        return None
    s = sorted(values)
    n = len(s)
    lo = 1 if n >= 20 else 0
    hi = -2 if n >= 20 else -1
    return statistics.median(s), s[lo], s[hi]


def geomean(values):
    positive = [v for v in values if v > 0]
    if not positive:
        return None
    return math.exp(sum(math.log(v) for v in positive) / len(positive))


def t975(df):
    """Cornish-Fisher approximation of the two-sided 95% Student-t quantile."""
    z = statistics.NormalDist().inv_cdf(0.975)
    return (z
            + (z ** 3 + z) / (4 * df)
            + (5 * z ** 5 + 16 * z ** 3 + 3 * z) / (96 * df ** 2)
            + (3 * z ** 7 + 19 * z ** 5 + 17 * z ** 3 - 15 * z) / (384 * df ** 3))


def welch_log_ratio(opp_raw, sc_raw):
    """95% interval for exp(mean(log(opp/sc))) using paired log-ratios."""
    pairs = [math.log(o / s) for o, s in zip(opp_raw, sc_raw) if o > 0 and s > 0]
    n = len(pairs)
    if n < 2:
        return None
    mean = statistics.fmean(pairs)
    sd = statistics.stdev(pairs)
    if sd == 0:
        return math.exp(mean), math.exp(mean), math.exp(mean)
    half = t975(n - 1) * sd / math.sqrt(n)
    return math.exp(mean), math.exp(mean - half), math.exp(mean + half)


def verdict(low, high):
    """Contract decision rule for ratio = comparator / SC."""
    if low is None or high is None:
        return "SKIP"
    if low > WIN_LOW:
        return "WIN"       # SC faster than comparator by >= 5% with CI excluding 1
    if high < REGRESSION_HIGH:
        return "REG"       # SC slower than comparator by >= 5% with CI excluding 1
    return "tie"


def fmt_ns(value):
    if value is None:
        return "n/a"
    if value <= FOLDED_NS:
        return "<=500ns*"
    if value < 1_000:
        return f"{value:.0f}ns"
    if value < 1_000_000:
        return f"{value / 1_000:.1f}us"
    if value < 1_000_000_000:
        return f"{value / 1_000_000:.2f}ms"
    return f"{value / 1_000_000_000:.2f}s"


def fmt_ratio(stats):
    if stats is None:
        return "n/a"
    r, lo, hi = stats
    return f"{r:.2f} [{lo:.2f}-{hi:.2f}]"


def load(path: Path):
    benchmarks = OrderedDict()
    with open(path, "r", encoding="utf-8", newline="") as fh:
        reader = csv.DictReader(fh)
        for lineno, row in enumerate(reader, start=2):
            bid = row["benchmark_id"]
            bench = benchmarks.setdefault(bid, {
                "group": row["group"],
                "name": row["name"],
                "systems": OrderedDict(),
            })
            system = row["system"]
            raw, bad = parse_rounds(row.get("raw_rounds_ns", ""))
            status = row.get("status", "")
            flags = []
            if status != "SUCCESS":
                flags.append(f"status={status}")
            if bad:
                flags.append(f"malformed rounds: {bad}")
            if status == "SUCCESS" and not raw:
                flags.append("SUCCESS without raw rounds")
            if status == "SUCCESS" and 0 < len(raw) < REQUIRED_ROUNDS:
                flags.append(f"only {len(raw)} rounds (contract requires >= {REQUIRED_ROUNDS})")
            if system in bench["systems"]:
                flags.append("duplicate benchmark/system row")
            med = statistics.median(raw) if raw else None
            if raw and row.get("median_ns"):
                try:
                    declared = int(row["median_ns"])
                    if med is not None and declared != med:
                        flags.append(f"median_ns {declared} != raw median {med}")
                except ValueError:
                    flags.append("non-integer median_ns")
            if raw and row.get("min_ns"):
                try:
                    declared = int(row["min_ns"])
                    if declared != min(raw):
                        flags.append(f"min_ns {declared} != raw min {min(raw)}")
                except ValueError:
                    flags.append("non-integer min_ns")
            bench["systems"][system] = {
                "status": status,
                "raw": raw,
                "median": med,
                "declared_median": row.get("median_ns"),
                "flags": flags,
                "csv_line": lineno,
            }
    return benchmarks


def compare(sc_cell, opp_cell):
    """Return (contract stats, contract verdict, welch stats, welch verdict, note)."""
    if not sc_cell or not opp_cell:
        return None, "SKIP", None, "SKIP", "missing cell"
    flags = sc_cell["flags"] + opp_cell["flags"]
    if sc_cell["status"] != "SUCCESS" or opp_cell["status"] != "SUCCESS":
        return None, "SKIP", None, "SKIP", f"status={opp_cell['status']}/{sc_cell['status']}"
    if not sc_cell["raw"] or not opp_cell["raw"]:
        return None, "SKIP", None, "SKIP", "missing rounds"
    sc_med = sc_cell["median"] if sc_cell["median"] is not None else sc_cell["declared_median"]
    opp_med = opp_cell["median"] if opp_cell["median"] is not None else opp_cell["declared_median"]
    sc_med = int(sc_med) if sc_med is not None else 0
    opp_med = int(opp_med) if opp_med is not None else 0
    if sc_med <= FOLDED_NS or opp_med <= FOLDED_NS:
        return None, "FOLD", None, "FOLD", f"folded cell (SC={sc_med}ns, opp={opp_med}ns)"
    n = min(len(sc_cell["raw"]), len(opp_cell["raw"]))
    ratios = [o / s for o, s in zip(opp_cell["raw"][:n], sc_cell["raw"][:n]) if s > 0]
    dropped = n - len(ratios)
    stats = order_stats(ratios)
    w = welch_log_ratio(opp_cell["raw"][:n], sc_cell["raw"][:n])
    note = f"paired n={len(ratios)}"
    if dropped:
        note += f" ({dropped} zero-denominator pairs dropped)"
    if any("mismatch" in f for f in flags):
        note += " [CSV summary mismatch]"
    return stats, verdict(*stats[1:]), (w if w else None), (verdict(*w[1:]) if w else "SKIP"), note


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--csv", type=Path, default=DEFAULT_CSV, help="results CSV to analyze")
    parser.add_argument("--verbose", action="store_true", help="list folded benchmarks and raw flags")
    args = parser.parse_args()
    path = args.csv
    if not path.exists():
        sys.stderr.write(f"Error: CSV not found: {path}\n")
        return 2

    benchmarks = load(path)
    print("NumLang benchmark analysis - frozen contract v1.0")
    print(f"CSV          : {path}")
    print(f"CSV SHA-256  : {sha256(path)}")
    print(f"Rows         : {sum(len(b['systems']) for b in benchmarks.values())} cells / "
          f"{len(benchmarks)} benchmarks")
    print("Decision rule: ratio = comparator/SC; WIN iff CI low > 1.05; "
          "REG iff CI high < 0.95; fold at <= 500 ns")

    short = {"NumLang-Base": "Base", "Rustc-O3": "Rustc", "MSVC-O2": "MSVC",
             "Clang-O3": "Clang", "GCC-O3": "GCC", "GHC-O2": "GHC"}
    hdr = f"{'benchmark':<16} {'SC med':>10} |"
    for name in COMPARATORS:
        lbl = short.get(name, name)
        hdr += f" {lbl + ' med':>10} {lbl + ' ratio [CI]':>22} {'V':>4} |"
    print()
    print(hdr)
    print("-" * len(hdr))

    results = {name: {} for name in COMPARATORS}
    for bid, bench in benchmarks.items():
        systems = bench["systems"]
        sc_cell = systems.get(SC)
        cells = [f"{bid:<16} {fmt_ns(sc_cell['median'] if sc_cell else None):>10} |"]
        for name in COMPARATORS:
            opp_cell = systems.get(name)
            stats, v, _w, _wv, _note = compare(sc_cell, opp_cell)
            results[name][bid] = {"stats": stats, "verdict": v}
            med = opp_cell["median"] if opp_cell else None
            cells.append(f" {fmt_ns(med):>10} {fmt_ratio(stats):>22} {v:>4} |")
        print("".join(cells))

    print()
    print("SUMMARY (ratios = comparator / NumLang-SC; geomean over non-folded, evaluable benchmarks)")
    print(f"{'comparator':<15} {'WIN':>4} {'tie':>4} {'REG':>4} {'FOLD':>5} {'SKIP':>5} "
          f"{'geomean median-ratio':>21} {'geomean median quotient':>24}")
    for name in COMPARATORS:
        counts = Counter(r["verdict"] for r in results[name].values())
        ratios = [r["stats"][0] for r in results[name].values() if r["stats"]]
        sc_cell = None
        quotients = []
        for bid, r in results[name].items():
            if not r["stats"]:
                continue
            sys_map = benchmarks[bid]["systems"]
            sm = sys_map[SC]["median"]
            om = sys_map[name]["median"]
            if sm and om and sm > FOLDED_NS and om > FOLDED_NS:
                quotients.append(om / sm)
        gm_ratio = geomean(ratios)
        gm_quot = geomean(quotients)
        print(f"{name:<15} {counts['WIN']:>4} {counts['tie']:>4} {counts['REG']:>4} "
              f"{counts['FOLD']:>5} {counts['SKIP']:>5} "
              f"{(f'{gm_ratio:.2f}x' if gm_ratio else 'n/a'):>21} "
              f"{(f'{gm_quot:.2f}x' if gm_quot else 'n/a'):>24}")

    print()
    print("ROBUSTNESS CROSS-CHECK - Welch 95% CI on paired log-ratios (not used for contract counts)")
    print(f"{'comparator':<15} {'WIN':>4} {'tie':>4} {'REG':>4} {'FOLD':>5} {'SKIP':>5} "
          f"{'verdict changes vs contract estimator':>40}")
    for name in COMPARATORS:
        wv = {}
        changed = []
        for bid in benchmarks:
            sc_cell = benchmarks[bid]["systems"].get(SC)
            opp_cell = benchmarks[bid]["systems"].get(name)
            _s, cv, w, w_verdict, _note = compare(sc_cell, opp_cell)
            wv[bid] = w_verdict
            if cv != w_verdict:
                changed.append(f"{bid}:{cv}->{w_verdict}")
        counts = Counter(wv.values())
        print(f"{name:<15} {counts['WIN']:>4} {counts['tie']:>4} {counts['REG']:>4} "
              f"{counts['FOLD']:>5} {counts['SKIP']:>5} "
              f"{(', '.join(changed) if changed else 'none'):>40}")

    print()
    print("DATA QUALITY FLAGS")
    status_counts = Counter()
    other_flags = []
    for bid, bench in benchmarks.items():
        for system, cell in bench["systems"].items():
            for flag in cell["flags"]:
                if flag.startswith("status="):
                    status_counts[(system, flag)] += 1
                else:
                    other_flags.append((bid, system, flag))
    for (system, flag), n in sorted(status_counts.items()):
        print(f"  {system:<15} {n:>2} cells: {flag}")
    if not status_counts:
        print("  none")
    print(f"  non-SUCCESS cells: {sum(status_counts.values())}")
    print(f"  other flags (missing rounds / traceability): {len(other_flags)}")
    for bid, system, flag in other_flags:
        print(f"    {bid} [{system}]: {flag}")

    if args.verbose:
        print()
        print("FOLDED BENCHMARKS (per comparator)")
        for name in COMPARATORS:
            folded = [bid for bid, r in results[name].items() if r["verdict"] == "FOLD"]
            print(f"  {name:<15}: {', '.join(folded) if folded else 'none'}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
