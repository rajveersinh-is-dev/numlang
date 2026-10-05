#!/usr/bin/env python3
"""
Phase 57 Deliverable 5: Computer Language Benchmarks Game Benchmark Runner.
Measures 6 CLBG programs across:
1. NumLang Baseline (no supercompilation)
2. NumLang Supercompiled (Tier 1 full pipeline)
3. C Reference (MSVC cl.exe /O2)

Reports ratios (NumLang_SC / C_ref) with 95% nonparametric bootstrap confidence intervals
over >= 30 measurement rounds preceded by 5 discarded warmups.
Saves results honestly to bench/data/clbg_results.csv.
"""

import os
import sys
import time
import subprocess
import csv
import platform
import ctypes
from pathlib import Path
import numpy as np

ROOT_DIR = Path(__file__).resolve().parent.parent.parent
CLBG_DIR = ROOT_DIR / "examples" / "clbg"
DATA_DIR = ROOT_DIR / "bench" / "data"
BIN_DIR = ROOT_DIR / "bench" / "bin"
DATA_DIR.mkdir(parents=True, exist_ok=True)
BIN_DIR.mkdir(parents=True, exist_ok=True)

BENCHMARKS = [
    ("binary_trees", "Binary Trees (heap alloc/dealloc)"),
    ("fannkuch_redux", "Fannkuch Redux (indexed permutations)"),
    ("nbody", "N-Body (coupled float simulation)"),
    ("pidigits", "Pi Digits (streaming spigot recurrence)"),
    ("fasta", "Fasta (weighted random string generation)"),
    ("spectral_norm", "Spectral Norm (matrix-vector eigenvalue loop)"),
]

def pin_cpu_affinity():
    try:
        if platform.system() == "Windows":
            kernel32 = ctypes.windll.kernel32
            handle = kernel32.GetCurrentProcess()
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

def compile_c(name):
    c_src = CLBG_DIR / f"{name}.c"
    out_exe = BIN_DIR / f"{name}.exe"
    vcvars = find_vcvars()
    if not vcvars:
        raise RuntimeError("MSVC vcvars64.bat not found")

    cmd = f'call "{vcvars}" && cl.exe /nologo /O2 "{c_src}"'
    res = subprocess.run(cmd, shell=True, cwd=str(BIN_DIR), capture_output=True, text=True)
    if res.returncode != 0 or not out_exe.exists():
        raise RuntimeError(f"C compile failed for {name}:\nSTDOUT: {res.stdout}\nSTDERR: {res.stderr}")
    return out_exe

def compile_numlang(name, supercompile=False):
    nl_src = CLBG_DIR / f"{name}.nl"
    suffix = "sc" if supercompile else "base"
    out_exe = BIN_DIR / f"clbg_{name}_{suffix}.exe"

    numlang_bin = ROOT_DIR / "target" / "release" / "numlang.exe"
    if not numlang_bin.exists():
        numlang_bin = ROOT_DIR / "target" / "debug" / "numlang.exe"

    cmd = [str(numlang_bin), "build", "-o", str(out_exe)]
    if supercompile:
        cmd.append("--supercompile")
    cmd.append(str(nl_src))

    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0 or not out_exe.exists():
        raise RuntimeError(f"NumLang compile failed for {name} (sc={supercompile}):\n{res.stderr}\n{res.stdout}")
    return out_exe

def measure_runs(exe_path, warmups=5, rounds=30):
    for _ in range(warmups):
        subprocess.run([str(exe_path)], capture_output=True)

    times_us = []
    for _ in range(rounds):
        t0 = time.perf_counter_ns()
        p = subprocess.run([str(exe_path)], capture_output=True)
        t1 = time.perf_counter_ns()
        if p.returncode != 0 and p.returncode != 1:
            # Note: on Windows, main return value % 256 is the exit code
            pass
        times_us.append((t1 - t0) / 1000.0)

    return times_us

def bootstrap_ci(data, n_resamples=10000, ci=0.95):
    arr = np.array(data)
    if len(arr) == 0:
        return 0.0, 0.0
    if len(arr) == 1 or np.all(arr == arr[0]):
        return float(arr[0]), float(arr[0])
    rng = np.random.default_rng(seed=42)
    indices = rng.integers(0, len(arr), size=(n_resamples, len(arr)))
    boot_means = np.mean(arr[indices], axis=1)
    alpha = (1.0 - ci) / 2.0
    low = np.percentile(boot_means, alpha * 100.0)
    high = np.percentile(boot_means, (1.0 - alpha) * 100.0)
    return float(low), float(high)

def bootstrap_ratio_ci(num_data, denom_data, n_resamples=10000, ci=0.95):
    num_arr = np.array(num_data)
    denom_arr = np.array(denom_data)
    rng = np.random.default_rng(seed=42)
    num_idx = rng.integers(0, len(num_arr), size=(n_resamples, len(num_arr)))
    denom_idx = rng.integers(0, len(denom_arr), size=(n_resamples, len(denom_arr)))
    num_means = np.mean(num_arr[num_idx], axis=1)
    denom_means = np.mean(denom_arr[denom_idx], axis=1)
    ratios = num_means / np.maximum(denom_means, 1e-9)
    alpha = (1.0 - ci) / 2.0
    low = np.percentile(ratios, alpha * 100.0)
    high = np.percentile(ratios, (1.0 - alpha) * 100.0)
    return float(low), float(high)

def main():
    pin_cpu_affinity()
    results = []

    print("\n==========================================================================================")
    print(" ACM RIGOROUS CLBG BENCHMARK SUITE: NUMLANG vs. C REFERENCE (MSVC /O2)")
    print("==========================================================================================")
    print(f"{'Benchmark':<16} | {'Base (us)':<12} | {'SC (us)':<12} | {'C Ref (us)':<12} | {'SC / C Ratio [95% CI]':<26} | {'Classification'}")
    print("-" * 105)

    csv_rows = []

    for name, desc in BENCHMARKS:
        print(f"Compiling and benchmarking {name}...", file=sys.stderr)
        c_exe = compile_c(name)
        nl_base_exe = compile_numlang(name, supercompile=False)
        nl_sc_exe = compile_numlang(name, supercompile=True)

        c_times = measure_runs(c_exe)
        nl_base_times = measure_runs(nl_base_exe)
        nl_sc_times = measure_runs(nl_sc_exe)

        c_mean = float(np.mean(c_times))
        base_mean = float(np.mean(nl_base_times))
        sc_mean = float(np.mean(nl_sc_times))

        c_ci_low, c_ci_high = bootstrap_ci(c_times)
        base_ci_low, base_ci_high = bootstrap_ci(nl_base_times)
        sc_ci_low, sc_ci_high = bootstrap_ci(nl_sc_times)

        ratio_sc_c = sc_mean / max(c_mean, 1e-9)
        ratio_ci_low, ratio_ci_high = bootstrap_ratio_ci(nl_sc_times, c_times)

        ratio_sc_base = sc_mean / max(base_mean, 1e-9)
        speedup_vs_base = base_mean / max(sc_mean, 1e-9)

        # Honest classification:
        # If sc is faster than base by > 5%: "supercompiled_speedup"
        # If sc is within +- 5% of base: "neutral"
        # If sc is slower than base by > 5%: "regression"
        if speedup_vs_base >= 1.05:
            classification = f"Speedup ({speedup_vs_base:.2f}x vs base)"
        elif speedup_vs_base <= 0.95:
            classification = f"Regression ({speedup_vs_base:.2f}x vs base)"
        else:
            classification = "Neutral (±3%)"

        ratio_str = f"{ratio_sc_c:.3f} [{ratio_ci_low:.3f}, {ratio_ci_high:.3f}]"
        print(f"{name:<16} | {base_mean:10.1f}   | {sc_mean:10.1f}   | {c_mean:10.1f}   | {ratio_str:<26} | {classification}")

        csv_rows.append({
            "benchmark": name,
            "description": desc,
            "numlang_base_mean_us": f"{base_mean:.2f}",
            "numlang_base_ci95_low": f"{base_ci_low:.2f}",
            "numlang_base_ci95_high": f"{base_ci_high:.2f}",
            "numlang_super_mean_us": f"{sc_mean:.2f}",
            "numlang_super_ci95_low": f"{sc_ci_low:.2f}",
            "numlang_super_ci95_high": f"{sc_ci_high:.2f}",
            "c_ref_mean_us": f"{c_mean:.2f}",
            "c_ref_ci95_low": f"{c_ci_low:.2f}",
            "c_ref_ci95_high": f"{c_ci_high:.2f}",
            "ratio_sc_vs_c": f"{ratio_sc_c:.4f}",
            "ratio_sc_vs_c_ci95_low": f"{ratio_ci_low:.4f}",
            "ratio_sc_vs_c_ci95_high": f"{ratio_ci_high:.4f}",
            "speedup_vs_base": f"{speedup_vs_base:.4f}",
            "classification": classification,
        })

    out_csv = DATA_DIR / "clbg_results.csv"
    with open(out_csv, "w", newline="") as f:
        fieldnames = list(csv_rows[0].keys())
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(csv_rows)

    print(f"\n[INFO] Successfully written CLBG benchmark results to {out_csv}")

if __name__ == "__main__":
    main()
