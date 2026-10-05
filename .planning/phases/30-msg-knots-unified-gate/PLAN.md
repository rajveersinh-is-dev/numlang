# Phase 30: Supercompiler Refinements (MSG Knots, Zero-Edge Leaves, Unified Gate) — Plan

> **Phase**: 30
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 30
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Materialize MSG generalized states as allocated knot target nodes, handle budget-exhaustion zero-edge non-return leaves safely with `Terminator::Unreachable`, precompute inliner loop functions, and unify the profitability gate.

## Root Cause / Motivation
Process tree residualization could encounter dangling knot targets when MSG generalized states were not materialized as distinct nodes, and unclosed leaf paths caused CFG generation anomalies.

## Requirements
- **REFIN-01**: Materialize MSG generalized state as fully allocated knot target nodes in process tree.
- **REFIN-02**: Handle budget-exhaustion zero-edge leaves safely by emitting `Terminator::Unreachable`.
- **REFIN-03**: Precompute `has_loop_funcs` set in AST inliner to eliminate redundant full-AST scans.
- **REFIN-04**: Unify profitability gate across Classic, Distill, and MRSC supercompiler modes.

## Key Deliverables
- `src/mir/supercompiler/residualize.rs`
- `src/mir/supercompiler/generalize.rs`
- `src/opt/inlining.rs`
- `tests/supercompiler_phase30_tests.rs`

## Verification
- `cargo test --test supercompiler_phase30_tests` passes 100%.
- Process tree correctly links MSG knot targets without dangling blocks.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
