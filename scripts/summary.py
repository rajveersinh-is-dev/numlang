#!/usr/bin/env python3
import csv
from pathlib import Path

csv_path = Path("bench/data/results.csv")
if not csv_path.exists():
    print("No results.csv found.")
    exit(0)

print(f"{'Benchmark':<18} | {'NumLang Base':<14} | {'NumLang Super':<14} | {'Rust -O3':<14} | {'C /O2':<14}")
print("-" * 84)

rows = {}
with open(csv_path, mode="r", newline="") as f:
    reader = csv.DictReader(f)
    for row in reader:
        b = row["benchmark"]
        cfg = row["config"]
        med = float(row["median_us"])
        if b not in rows:
            rows[b] = {}
        rows[b][cfg] = med

for b, cfgs in rows.items():
    nl_b = f"{cfgs.get('numlang_base', 0.0):.1f} us" if 'numlang_base' in cfgs else "N/A"
    nl_s = f"{cfgs.get('numlang_super', 0.0):.1f} us" if 'numlang_super' in cfgs else "N/A"
    rs   = f"{cfgs.get('rust_opt', 0.0):.1f} us" if 'rust_opt' in cfgs else "N/A"
    c    = f"{cfgs.get('c_opt', 0.0):.1f} us" if 'c_opt' in cfgs else "N/A"
    print(f"{b:<18} | {nl_b:<14} | {nl_s:<14} | {rs:<14} | {c:<14}")

print("=" * 84)
print("Reproduction complete. See bench/figures/ and paper/ for paper draft.")
