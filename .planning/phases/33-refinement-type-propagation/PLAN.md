# Phase 33: Refinement Type Propagation Through Process Tree — Plan

> **Phase**: 33
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 33
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Add `Interval` arithmetic and `refinements` mapping to `SymbolicState`, narrow branch intervals across 6 comparison operators, prune dead branches on empty intervals, and eliminate array bounds checks.

## Root Cause / Motivation
Supercompilation without path-sensitive interval tracking generates residual code with redundant boundary tests and dead conditional branches.

## Requirements
- **REFINE-01**: Add `Interval` arithmetic and `refinements` mapping to `SymbolicState`.
- **REFINE-02**: Implement interval propagation across arithmetic operations (`Add`, `Sub`, `Mul`).
- **REFINE-03**: Implement branch narrowing across 6 comparison operators (`<`, `<=`, `>`, `>=`, `==`, `!=`).
- **REFINE-04**: Prune dead branches when intervals become empty; propagate call-site argument refinements into inlined callees.
- **REFINE-05**: Supercompiler-level bounds-check elimination (`sc_bce_eliminated`).

## Key Deliverables
- `src/mir/supercompiler/state.rs`
- `src/mir/supercompiler/drive.rs`
- `tests/supercompiler_phase33_tests.rs`

## Verification
- `cargo test --test supercompiler_phase33_tests` passes 5/5 tests green.
- Redundant branches and bounds checks pruned from supercompiled output.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
