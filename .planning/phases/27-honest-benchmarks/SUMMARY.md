# Phase 27 Summary: Honest High-Precision Benchmarks & Supercompiler Comparisons

> **Phase**: 27
> **Status**: Completed
> **Traceability**: Requirements `BENCH-01` .. `BENCH-04`, Master Plan §8
> **Governing Standards**: `INTEGRITY_RULES.md`, `GEMINI.md`

---

## 1. Executive Summary

Phase 27 established an in-process, high-precision benchmarking infrastructure in `bench/harness/runner.py`.

High-resolution hardware performance counters (`QueryPerformanceCounter` on Windows, `clock_gettime(CLOCK_MONOTONIC)` on POSIX) are sampled in-process over $\ge 10,000$ iterations, preceded by discarded warmup runs to eliminate process creation and OS scheduling jitter.

Baseline C benchmarks were audited and corrected, including resolving a double-free bug in `append3.c`. NumLang was evaluated head-to-head against classical literature supercompilers (SPSC and HOSC) with strict exit code validation.

## 2. Changes by File

| File | Change Summary |
| :--- | :--- |
| `bench/harness/runner.py` | Rewrote benchmarking runner with high-resolution in-process hardware timers, discarded warmup iterations, and strict exit code validation. |
| `bench/c/append3.c` | Fixed memory corruption and double-free in baseline C implementation. |
| `tests/supercompiler_head_to_head.rs` | Added head-to-head evaluation test suite comparing NumLang against SPSC and HOSC. |

## 3. Compliance with Governing Rules

1. **NO PRELOADING OF NUMBERS / PRECOMPUTED CONSTANTS**:
   - Workloads execute real computations to completion; timings reflect hardware performance counters directly.
2. **ZERO BENCHMARK NAME COUPLING**:
   - The benchmark harness runs external executables agnostically without special casing.
3. **VERIFIABILITY, PURITY & TYPE SAFETY**:
   - Zero `.unwrap()` in lowering pipelines, zero `panic!()`, zero `#[allow(...)]` suppressions.
   - Built and passed `cargo clippy --all-targets -- -D warnings` with 0 errors and 0 warnings.
