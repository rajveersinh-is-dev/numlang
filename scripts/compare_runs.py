#!/usr/bin/env python3
"""Compare two showdown CSV snapshots kernel-by-kernel.

Usage: python scripts/compare_runs.py BEFORE.csv AFTER.csv
Prints, for each benchmark, the SC median change and any WIN/tie/REG verdict
changes across the six comparators, then a summary of count changes.
"""

import csv
import sys
from collections import OrderedDict
from pathlib import Path

FOLDED_NS = 500


def load(path):
    rows = OrderedDict()
    with open(path, "r", encoding="utf-8") as f:
        for r in csv.DictReader(f):
            key = (r["name"], r["system"])
            rows[key] = r
    return rows


def verdict(row, sc_ns):
    if row.get("status") != "SUCCESS":
        return row.get("status", "?")[:4]
    try:
        med = int(row["median_ns"])
    except (ValueError, TypeError):
        return "?"
    if med <= FOLDED_NS:
        return "FOLD"
    raw = [int(x) for x in row.get("raw_rounds_ns", "").split(";") if x.strip().isdigit()]
    scraw = sc_ns
    if not raw or not scraw:
        return "tie"
    n = min(len(raw), len(scraw))
    ratios = sorted(raw[i] / scraw[i] for i in range(n) if scraw[i] > 0)
    if len(ratios) < 4:
        return "tie"
    lo, hi = ratios[1], ratios[-2]
    if lo > 1.05:
        return "WIN"
    if hi < 0.95:
        return "REG"
    return "tie"


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    before, after = load(sys.argv[1]), load(sys.argv[2])
    names = []
    for (name, system) in before:
        if system == "NumLang-SC" and name not in names:
            names.append(name)
    comparators = ["NumLang-Base", "Rustc-O3", "MSVC-O2", "Clang-O3", "GCC-O3", "GHC-O2"]
    changed = 0
    for name in names:
        sc_key = (name, "NumLang-SC")
        b_sc = before.get(sc_key)
        a_sc = after.get(sc_key)
        if not a_sc:
            continue
        b_med = int(b_sc["median_ns"]) if b_sc else 0
        a_med = int(a_sc["median_ns"])
        delta = (a_med - b_med) / b_med * 100 if b_med else 0.0
        notes = []
        scraw = [int(x) for x in a_sc.get("raw_rounds_ns", "").split(";") if x.strip().isdigit()]
        for comp in comparators:
            b = verdict(before[(name, comp)], [int(x) for x in b_sc.get("raw_rounds_ns", "").split(";") if x.strip().isdigit()] if b_sc else [])
            a = verdict(after[(name, comp)], scraw)
            if a != b:
                notes.append(f"{comp}:{b}->{a}")
        flag = "  <<<" if notes or abs(delta) >= 5 else ""
        if notes or abs(delta) >= 5:
            changed += 1
        print(f"{name:<18} SC {b_med/1000:7.2f}us -> {a_med/1000:7.2f}us ({delta:+6.1f}%) {' '.join(notes)}{flag}")
    print(f"\n{changed} of {len(names)} kernels changed materially (>=5% median or verdict flip)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
