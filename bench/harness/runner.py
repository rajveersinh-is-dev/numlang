#!/usr/bin/env python3
"""
ACM SIGPLAN Rigorous Empirical Benchmark Harness
Measures NumLang (Baseline vs Supercompiled) against Rust, C, and Haskell (GHC).
Enforces:
- CPU core pinning (SetProcessAffinityMask / taskset)
- 5 warm-up runs (discarded) + 30 timed measurement runs
- Nonparametric 10,000-sample bootstrap 95% confidence intervals
- Mann-Whitney U hypothesis testing (alpha = 0.01)
- 3 evaluation dimensions: Runtime speed, Compile-time throughput, Residual code size
"""

import os
import sys
import time
import subprocess
import csv
import math
import statistics
import platform
import ctypes
from pathlib import Path
import numpy as np
from scipy import stats

ROOT_DIR = Path(__file__).resolve().parent.parent.parent
BENCH_DIR = ROOT_DIR / "bench"
DATA_DIR = BENCH_DIR / "data"
DATA_DIR.mkdir(parents=True, exist_ok=True)
TEMP_BIN_DIR = BENCH_DIR / "bin"
TEMP_BIN_DIR.mkdir(parents=True, exist_ok=True)

BENCHMARKS = [
    "nrev",
    "append3",
    "stream_fusion",
    "ackermann",
    "fib_matrix",
    "sieve",
    "matvec_4x4",
    "raytracer_sphere",
    "tree_flip",
    "peano_mul",
]

def pin_cpu_affinity():
    """Pin the runner process to a single CPU core to prevent thread migration."""
    try:
        if platform.system() == "Windows":
            kernel32 = ctypes.windll.kernel32
            handle = kernel32.GetCurrentProcess()
            # Pin to CPU 0 (mask = 1)
            kernel32.SetProcessAffinityMask(handle, 1)
            print("[INFO] Pinned process affinity to CPU Core 0 (Windows)")
        elif hasattr(os, "sched_setaffinity"):
            os.sched_setaffinity(0, {0})
            print("[INFO] Pinned process affinity to CPU Core 0 (Linux)")
    except Exception as e:
        print(f"[WARN] Failed to set CPU affinity: {e}")

def find_vcvars():
    candidates = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\VC\Auxiliary\Build\vcvars64.bat",
    ]
    for c in candidates:
        if os.path.exists(c):
            return c
    return None

def compile_benchmark(name, config):
    exe_path = TEMP_BIN_DIR / f"{name}_{config}.exe"
    src_nl = BENCH_DIR / "numlang" / f"{name}.nl"
    src_rs = BENCH_DIR / "rust" / f"{name}.rs"
    src_c = BENCH_DIR / "c" / f"{name}.c"
    src_hs = BENCH_DIR / "haskell" / f"{name}.hs"

    numlang_bin = ROOT_DIR / "target" / "release" / "numlang.exe"
    if not numlang_bin.exists():
        numlang_bin = ROOT_DIR / "target" / "debug" / "numlang.exe"

    t0 = time.perf_counter()

    if config == "numlang_base":
        cmd = [str(numlang_bin), "build", "--backend", "cranelift", "-o", str(exe_path), str(src_nl)]
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(ROOT_DIR))
        compile_time_ms = (time.perf_counter() - t0) * 1000.0
        if res.returncode != 0 or not exe_path.exists():
            return None, 0.0, 0
        return exe_path, compile_time_ms, exe_path.stat().st_size

    elif config == "numlang_super":
        cmd = [str(numlang_bin), "build", "--supercompile", "--backend", "cranelift", "-o", str(exe_path), str(src_nl)]
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(ROOT_DIR))
        compile_time_ms = (time.perf_counter() - t0) * 1000.0
        if res.returncode != 0 or not exe_path.exists():
            return None, 0.0, 0
        return exe_path, compile_time_ms, exe_path.stat().st_size

    elif config == "rust_opt":
        cmd = ["rustc", "-C", "opt-level=3", "-C", "codegen-units=1", "-o", str(exe_path), str(src_rs)]
        res = subprocess.run(cmd, capture_output=True, text=True)
        compile_time_ms = (time.perf_counter() - t0) * 1000.0
        if res.returncode != 0 or not exe_path.exists():
            return None, 0.0, 0
        return exe_path, compile_time_ms, exe_path.stat().st_size

    elif config == "c_opt":
        vcvars = find_vcvars()
        if vcvars:
            bat_cmd = f'call "{vcvars}" >nul 2>&1 && cl.exe /O2 /Fe:"{exe_path}" "{src_c}" >nul 2>&1'
            res = subprocess.run(bat_cmd, shell=True, capture_output=True, text=True)
        else:
            cmd = ["clang", "-O3", "-march=native", "-o", str(exe_path), str(src_c)]
            res = subprocess.run(cmd, capture_output=True, text=True)
        compile_time_ms = (time.perf_counter() - t0) * 1000.0
        if not exe_path.exists():
            return None, 0.0, 0
        return exe_path, compile_time_ms, exe_path.stat().st_size

    elif config == "ghc_opt":
        cmd = ["ghc", "-O2", str(src_hs), "-o", str(exe_path)]
        res = subprocess.run(cmd, capture_output=True, text=True)
        compile_time_ms = (time.perf_counter() - t0) * 1000.0
        if not exe_path.exists():
            return None, 0.0, 0
        return exe_path, compile_time_ms, exe_path.stat().st_size

    return None, 0.0, 0

def measure_execution_times(exe_path, warmups=5, rounds=30):
    """Executes a binary with warmups and collects high-resolution durations in microseconds."""
    for _ in range(warmups):
        subprocess.run([str(exe_path)], capture_output=True)

    times_us = []
    for _ in range(rounds):
        t0 = time.perf_counter_ns()
        p = subprocess.run([str(exe_path)], capture_output=True)
        t1 = time.perf_counter_ns()
        times_us.append((t1 - t0) / 1000.0)

    return times_us

def bootstrap_ci(data, n_resamples=10000, ci=0.95):
    """Nonparametric bootstrap confidence interval for the mean."""
    arr = np.array(data)
    boot_means = np.random.choice(arr, size=(n_resamples, len(arr)), replace=True).mean(axis=1)
    lower = np.percentile(boot_means, (1.0 - ci) / 2.0 * 100.0)
    upper = np.percentile(boot_means, (1.0 + ci) / 2.0 * 100.0)
    return lower, upper

def main():
    pin_cpu_affinity()
    print("=================================================================")
    print(" ACM SIGPLAN Benchmark Harness: Multi-Compiler Empirical Study ")
    print("=================================================================")

    configs = ["numlang_base", "numlang_super", "rust_opt", "c_opt"]
    # Check if ghc is installed
    try:
        subprocess.run(["ghc", "--version"], capture_output=True)
        configs.append("ghc_opt")
    except FileNotFoundError:
        pass

    results_csv_path = DATA_DIR / "results.csv"
    fieldnames = [
        "benchmark",
        "config",
        "mean_us",
        "median_us",
        "std_dev_us",
        "iqr_us",
        "ci_lower_us",
        "ci_upper_us",
        "compile_time_ms",
        "binary_size_bytes",
        "speedup_vs_baseline",
        "mann_whitney_p",
    ]

    records = []

    for bench in BENCHMARKS:
        print(f"\n[BENCHMARK] >>> {bench}")
        baseline_times = None

        for cfg in configs:
            exe_path, comp_time, bin_size = compile_benchmark(bench, cfg)
            if not exe_path or not exe_path.exists():
                print(f"  [SKIPPED] {cfg} for {bench} (compilation failed or toolchain missing)")
                continue

            times = measure_execution_times(exe_path, warmups=5, rounds=30)
            mean_val = float(np.mean(times))
            median_val = float(np.median(times))
            std_val = float(np.std(times, ddof=1)) if len(times) > 1 else 0.0
            iqr_val = float(stats.iqr(times))
            ci_low, ci_high = bootstrap_ci(times)

            speedup = 1.0
            p_val = 1.0

            if cfg == "numlang_base":
                baseline_times = times
            elif baseline_times is not None:
                speedup = float(np.mean(baseline_times)) / mean_val if mean_val > 0 else 1.0
                try:
                    stat_res = stats.mannwhitneyu(baseline_times, times, alternative='two-sided')
                    p_val = float(stat_res.pvalue)
                except Exception:
                    p_val = 1.0

            print(f"  [{cfg:14s}] Mean: {mean_val:8.2f} us | Median: {median_val:8.2f} us | 95% CI: [{ci_low:7.2f}, {ci_high:7.2f}] | Size: {bin_size:7d} B | Speedup: {speedup:5.2f}x")

            records.append({
                "benchmark": bench,
                "config": cfg,
                "mean_us": round(mean_val, 2),
                "median_us": round(median_val, 2),
                "std_dev_us": round(std_val, 2),
                "iqr_us": round(iqr_val, 2),
                "ci_lower_us": round(ci_low, 2),
                "ci_upper_us": round(ci_high, 2),
                "compile_time_ms": round(comp_time, 2),
                "binary_size_bytes": bin_size,
                "speedup_vs_baseline": round(speedup, 3),
                "mann_whitney_p": p_val,
            })

    with open(results_csv_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(records)

    print(f"\n[DONE] Results written to {results_csv_path}")

if __name__ == "__main__":
    main()
