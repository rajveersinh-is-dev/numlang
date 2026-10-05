# Phase 29: Fix Supercompiler Regressions — Plan

> **Phase**: 29
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 29
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Resolve 4 confirmed benchmark regressions: Ackermann (depth budget & knot-tying), stream_fusion (loop-invariant call guards), fib_matrix (MSG fallback), and power_spec (profitability gate bailing on zero reductions).

## Root Cause / Motivation
Advancements in distillation and generalization uncovered corner cases where deep recurrences exhausted budgets or inlining inside loop bodies led to code bloat.

## Requirements
- **REGRESS-01**: Fix Ackermann regression by implementing call-site depth budgeting and knot-tying fallback.
- **REGRESS-02**: Fix stream_fusion regression by enforcing loop-invariant call-site guards and disabling loop-inlining inside existing loops.
- **REGRESS-03**: Fix fib_matrix regression by adding MSG anti-unification fallback on recurrence failure.
- **REGRESS-04**: Fix power_spec regression by refining profitability gate to prevent bailing on zero reductions.

## Key Deliverables
- `src/mir/supercompiler/mod.rs`
- `src/mir/supercompiler/drive.rs`
- `src/mir/supercompiler/generalize.rs`
- `tests/supercompiler_regression_fix_tests.rs`

## Verification
- `cargo test --test supercompiler_regression_fix_tests` passes 100%.
- Zero regressions on Ackermann, stream_fusion, fib_matrix, and power_spec.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
