# Phase 39: Real-World Benchmark Dominance (30-Benchmark Expansion & Head-to-Head Comparison) — Plan

> **Phase**: 39
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 39
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Expand benchmark suite from 13 to 30 programs across 5 domains, extend `runner.py` with empirical bootstrap CI harness, generate verifiable LaTeX tables, and fix polyhedral loop buffer contraction safety.

## Root Cause / Motivation
Empirical evaluation required broad representation beyond micro-benchmarks, covering pattern matching, sorting, graph algorithms, scientific computing, and functional idioms.

## Requirements
- **DOM-01**: Expand benchmark suite from 13 to 30 programs across 5 domains (Pattern Matching, Sorting, Graph Algorithms, Numerical/Scientific Computing, Functional Idioms).
- **DOM-02**: Extend `runner.py` with `BenchmarkEntry` taxonomy and empirical bootstrap CI harness.
- **DOM-03**: Extend `generate_tables.py` with `generate_head_to_head_table()` and `generate_ablation_table()` emitting verifiable LaTeX tables.
- **DOM-04**: Update `paper/main.tex` §7 Evaluation.
- **DOM-05**: Fix polyhedral loop buffer contraction safety in `polyhedral.rs`.

## Key Deliverables
- `bench/numlang/*.nl` (30 programs)
- `bench/harness/runner.py`
- `bench/harness/generate_tables.py`
- `src/mir/supercompiler/polyhedral.rs`
- `tests/supercompiler_phase39_tests.rs`

## Verification
- `cargo test --test supercompiler_phase39_tests` passes 5/5 tests green.
- All 30 benchmarks execute to completion with returncode == 0.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
