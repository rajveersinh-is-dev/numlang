# Phase 16: Canonical Literature Benchmarks & Statistical Harness — Plan

> **Phase**: 16
> **Status**: Completed
> **Traceability**: Master Plan Part II, Requirements BENCH-HARN-01..04
> **Milestone**: Core Supercompiler & Language Extensions (Phases 10–19)

## Objective
Assemble 10 canonical supercompiler benchmarks from the literature, implemented across NumLang, Rust, C, and Haskell (GHC), accompanied by a statistical harness calculating 95% bootstrap confidence intervals.

## Requirements
- **BENCH-HARN-01**: Implement canonical workloads: `nrev`, `append3`, `stream_fusion`, `ackermann`, `fib_matrix`, `sieve`, `matvec_4x4`, `raytracer_sphere`, `tree_flip`, `peano_mul`.
- **BENCH-HARN-02**: Provide reference implementations in C (MSVC/Clang), Rust (`rustc -O`), and Haskell (GHC `-O2`).
- **BENCH-HARN-03**: Implement statistical benchmarking harness in `bench/harness/runner.py` with in-process microsecond timing, $\ge 5$ warmup iterations, and $\ge 30$ measurement iterations.
- **BENCH-HARN-04**: Calculate 95% bootstrap confidence intervals, geometric means, and export machine-readable CSV results.

## Key Deliverables
- `bench/benchmarks/`, `bench/harness/runner.py`, `bench/data/`

## Verification
- Benchmarking harness runs without errors, producing statistically valid measurements in `bench/data/results.csv`.
