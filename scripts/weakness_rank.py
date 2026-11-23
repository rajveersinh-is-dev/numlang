#!/usr/bin/env python3
"""Rank benchmark kernels by residual weakness for wave re-targeting.

Score = 10 per REG cell + 3 per tie cell + (margin penalty for narrow WINs)
        + log-scaled SC median (slow kernels have more absolute headroom).
Uses the same frozen contract estimator as scripts/analyze_results.py.
"""

import csv
import math
import sys
from collections import OrderedDict
from pathlib import Path

FOLDED_NS = 500
COMPARATORS = ["NumLang-Base", "Rustc-O3", "MSVC-O2", "Clang-O3", "GCC-O3", "GHC-O2"]


def ratio_stats(base_raw, sc_raw):
    if not base_raw or not sc_raw:
        return None
    n = min(len(base_raw), len(sc_raw))
    ratios = sorted(base_raw[i] / sc_raw[i] for i in range(n) if sc_raw[i] > 0)
    if len(ratios) < 4:
        return None
    return ratios[1], ratios[-2]


def main():
    csv_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("bench/data/showdown_results.csv")
    data = OrderedDict()
    with open(csv_path, "r", encoding="utf-8") as f:
        for r in csv.DictReader(f):
            data.setdefault(r["name"], {})[r["system"]] = r
    out = []
    for name, systems in data.items():
        sc = systems.get("NumLang-SC")
        if not sc or sc.get("status") != "SUCCESS":
            continue
        sc_ns = int(sc["median_ns"])
        scraw = [int(x) for x in sc.get("raw_rounds_ns", "").split(";") if x.strip().isdigit()]
        reg, ties, narrow, note = 0, 0, 0, []
        for comp in COMPARATORS:
            c = systems.get(comp)
            if not c or c.get("status") != "SUCCESS" or int(c.get("median_ns") or 0) <= FOLDED_NS:
                continue
            st = ratio_stats([int(x) for x in c.get("raw_rounds_ns", "").split(";") if x.strip().isdigit()], scraw)
            if not st:
                continue
            lo, hi = st
            if hi < 0.95:
                reg += 1
                note.append(f"REG:{comp}({hi:.2f})")
            elif lo <= 1.05:
                ties += 1
                note.append(f"tie:{comp}")
            elif lo < 1.5:
                narrow += 1
                note.append(f"narrow:{comp}({lo:.2f})")
        score = 10 * reg + 3 * ties + narrow + 2.0 * math.log1p(sc_ns / 1000.0)
        out.append((score, name, sc_ns, note))
    out.sort(reverse=True)
    print(f"{'rank':<5}{'kernel':<40}{'SC':>9}  {'score':>6}  issues")
    for i, (score, name, sc_ns, note) in enumerate(out, 1):
        print(f"{i:<5}{name:<40}{sc_ns/1000:7.1f}us  {score:6.1f}  {' '.join(note)}")


if __name__ == "__main__":
    main()
