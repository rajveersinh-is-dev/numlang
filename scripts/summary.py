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
        if row.get("status") == "CRASH":
            val_str = "CRASH"
        elif row.get("median_us"):
            val_str = f"{float(row['median_us']):.1f} us"
        else:
            val_str = "N/A"
        if b not in rows:
            rows[b] = {}
        rows[b][cfg] = val_str

for b, cfgs in rows.items():
    nl_b = cfgs.get('numlang_base', "N/A")
    nl_s = cfgs.get('numlang_super', "N/A")
    rs   = cfgs.get('rust_opt', "N/A")
    c    = cfgs.get('c_opt', "N/A")
    print(f"{b:<18} | {nl_b:<14} | {nl_s:<14} | {rs:<14} | {c:<14}")

print("=" * 84)
print("Reproduction complete. See bench/figures/ and paper/ for paper draft.")
