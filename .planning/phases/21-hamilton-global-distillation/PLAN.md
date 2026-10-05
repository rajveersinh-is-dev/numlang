# Phase 21: Real Hamilton Global Distillation — Plan

> **Phase**: 21
> **Status**: Completed
> **Traceability**: Master Plan §2, Requirements DISTILL-01..DISTILL-03
> **Milestone**: Remediation & Frontier (Phases 20–28)

## Objective
Replace structural hash DAG dedup with genuine Hamilton (2007) global process-tree distillation, eliminating intermediate heap structures across function boundaries.

## Root Cause / Motivation
The initial implementation only performed DAG hash deduplication on basic expressions, failing to unfold and deforest compositions across distinct recursive function call boundaries and leaving intermediate heap allocations.

## Requirements
- **DISTILL-01**: Implement a global process tree representation in `src/mir/supercompiler/distill.rs` modeling call configurations across the entire call graph.
- **DISTILL-02**: Implement a global whistle and inter-procedural folding mechanism across distinct function definitions.
- **DISTILL-03**: Verify automated deforestation of composed recursive functions (e.g., `append (append xs ys) zs` $\to$ single-pass 3-argument function without intermediate list allocations).

## Key Deliverables
- `src/mir/supercompiler/distill.rs`
- `tests/distillation_tests.rs`

## Verification
- `cargo test --test distillation_tests` passes 100%.
- Deforest nested recursive calls (`append(append(xs, ys), zs)`) into a single 3-argument function without intermediate heap allocations.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
