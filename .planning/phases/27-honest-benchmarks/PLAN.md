# Phase 27: Honest High-Precision Benchmarks & Supercompiler Comparisons — Plan

> **Phase**: 27
> **Status**: Completed
> **Traceability**: Master Plan §8, Requirements BENCH-01..BENCH-04
> **Milestone**: Remediation & Frontier (Phases 20–28)

## Objective
Overhaul benchmarking harness to eliminate OS process spawn artifacts, fix C baseline bugs, and compare directly against SPSC and HOSC on canonical benchmarks.

## Root Cause / Motivation
Process-spawn measurements distorted microbenchmark timings by orders of magnitude, and buggy C baselines compromised evaluation honesty.

## Requirements
- **BENCH-01**: Rewrite `bench/harness/runner.py` to use in-process microsecond hardware performance counter timing across $\ge 10,000$ iterations.
- **BENCH-02**: Validate process exit codes (`assert returncode == 0`) and report any crashes explicitly as `ERROR`.
- **BENCH-03**: Fix memory bugs in C baselines (fix `append3` double-free).
- **BENCH-04**: Benchmark NumLang head-to-head against SPSC and HOSC on canonical literature benchmarks (KMP, Wadler deforestation, Peano multiplication, etc.).

## Key Deliverables
- `bench/harness/runner.py`
- `tests/supercompiler_head_to_head.rs`
- `bench/c/append3.c`

## Verification
- `cargo test --test supercompiler_head_to_head` passes 100%.
- `runner.py` executes $\ge 10,000$ iterations with in-process timing and verifies exit code 0.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
