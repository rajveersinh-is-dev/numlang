#!/usr/bin/env python3
"""
benchmark_allocator_attribution.py: Measure Rust, C, and NumLang variants
under identical allocator strategies (Heap vs Arena) for deforestation benchmarks.
"""

import csv
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE_DIR = ROOT / "docs" / "evidence" / "round2"
EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)
OUT_CSV = EVIDENCE_DIR / "allocator_attribution.csv"


def compile_and_time(cmd_compile, exe_path):
    try:
        res = subprocess.run(cmd_compile, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
        if res.returncode != 0:
            return None, f"Compile failed: {res.stderr}"
    except Exception as e:
        return None, str(e)

    # 5 warmups
    for _ in range(5):
        subprocess.run([str(exe_path)], stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    # 30 measurement rounds
    times = []
    for _ in range(30):
        out = subprocess.run([str(exe_path)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        ns = None
        for line in out.stdout.splitlines():
            if line.startswith("COMPUTE_NS:"):
                try:
                    ns = int(line.split(":")[1].strip())
                except ValueError:
                    pass
        if ns is not None:
            times.append(ns)

    if not times:
        return None, "No COMPUTE_NS parsed"
    times.sort()
    median = times[len(times) // 2]
    min_val = times[0]
    p95 = times[int(len(times) * 0.95)]
    return {"min_ns": min_val, "median_ns": median, "p95_ns": p95, "raw": times}, None


def main():
    tmp_dir = Path(tempfile.mkdtemp(prefix="numlang_alloc_"))
    results = []

    benchmarks = ["nrev", "append3", "tree_flip"]

    for b in benchmarks:
        print(f"Benchmarking allocator variants for {b}...")

        # 1. Rust Heap
        rs_heap_src = ROOT / "bench" / "showdown" / "rust" / f"{b}.rs"
        rs_heap_exe = tmp_dir / f"{b}_rs_heap.exe"
        data, err = compile_and_time(["rustc", "-C", "opt-level=3", str(rs_heap_src), "-o", str(rs_heap_exe)], rs_heap_exe)
        if data:
            results.append({"benchmark": b, "system": "Rust", "allocator": "Heap (malloc/Box)", **data})
            print(f"  Rust Heap : {data['median_ns'] / 1000.0:.1f} µs")
        else:
            print(f"  Rust Heap error: {err}")

        # 2. Rust Arena
        rs_arena_src = ROOT / "bench" / "showdown" / "rust" / f"{b}_arena.rs"
        rs_arena_exe = tmp_dir / f"{b}_rs_arena.exe"
        data, err = compile_and_time(["rustc", "-C", "opt-level=3", str(rs_arena_src), "-o", str(rs_arena_exe)], rs_arena_exe)
        if data:
            results.append({"benchmark": b, "system": "Rust", "allocator": "Arena (static pool)", **data})
            print(f"  Rust Arena: {data['median_ns'] / 1000.0:.1f} µs")
        else:
            print(f"  Rust Arena error: {err}")

        # 3. C Arena
        c_arena_src = ROOT / "bench" / "showdown" / "c" / (f"{b}_arena.c" if (ROOT / "bench" / "showdown" / "c" / f"{b}_arena.c").exists() else f"{b}.c")
        c_arena_exe = tmp_dir / f"{b}_c_arena.exe"
        data, err = compile_and_time(["cl", "/O2", "/nologo", str(c_arena_src), f"/Fe:{c_arena_exe}"], c_arena_exe)
        if not data: # fallback to gcc or clang if cl is not on path directly
            data, err = compile_and_time(["gcc", "-O3", str(c_arena_src), "-o", str(c_arena_exe)], c_arena_exe)
        if data:
            results.append({"benchmark": b, "system": "C", "allocator": "Arena (static pool)", **data})
            print(f"  C Arena   : {data['median_ns'] / 1000.0:.1f} µs")

        # 4. C Heap
        c_heap_src = ROOT / "bench" / "showdown" / "c" / (f"{b}_malloc.c" if (ROOT / "bench" / "showdown" / "c" / f"{b}_malloc.c").exists() else f"{b}.c")
        c_heap_exe = tmp_dir / f"{b}_c_heap.exe"
        data, err = compile_and_time(["cl", "/O2", "/nologo", str(c_heap_src), f"/Fe:{c_heap_exe}"], c_heap_exe)
        if not data:
            data, err = compile_and_time(["gcc", "-O3", str(c_heap_src), "-o", str(c_heap_exe)], c_heap_exe)
        if data:
            results.append({"benchmark": b, "system": "C", "allocator": "Heap (malloc)", **data})
            print(f"  C Heap    : {data['median_ns'] / 1000.0:.1f} µs")

        # 5. NumLang Base
        nl_src = ROOT / "bench" / "showdown" / "numlang" / f"{b}.nl"
        nl_base_exe = tmp_dir / f"{b}_nl_base.exe"
        numlang_bin = ROOT / "target" / "release" / "numlang.exe"
        data, err = compile_and_time([str(numlang_bin), "build", str(nl_src), "--bench", "--use-mir", "-o", str(nl_base_exe)], nl_base_exe)
        if data:
            results.append({"benchmark": b, "system": "NumLang-Base", "allocator": "Runtime Heap/Arena", **data})
            print(f"  NumLang-Base: {data['median_ns'] / 1000.0:.1f} µs")

        # 6. NumLang SC
        nl_sc_exe = tmp_dir / f"{b}_nl_sc.exe"
        data, err = compile_and_time([str(numlang_bin), "build", str(nl_src), "--bench", "--supercompile", "-o", str(nl_sc_exe)], nl_sc_exe)
        if data:
            results.append({"benchmark": b, "system": "NumLang-SC", "allocator": "Runtime Heap/Arena (Deforested)", **data})
            print(f"  NumLang-SC  : {data['median_ns'] / 1000.0:.1f} µs")

    # Write CSV
    with open(OUT_CSV, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=["benchmark", "system", "allocator", "min_ns", "median_ns", "p95_ns", "raw"])
        writer.writeheader()
        for r in results:
            writer.writerow(r)

    print(f"Allocator attribution benchmark written to {OUT_CSV}")
    shutil.rmtree(tmp_dir, ignore_errors=True)


if __name__ == "__main__":
    main()
