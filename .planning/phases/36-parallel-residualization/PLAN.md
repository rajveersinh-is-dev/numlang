# Phase 36: Parallel Residualization (Independence Detection) — Plan

> **Phase**: 36
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 36
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Extend MIR with `Terminator::Fork`, detect independent subtrees and knots, and generate parallel execution forks targeting multithreaded runtime shims.

## Root Cause / Motivation
Independent branches in process trees (e.g. tree traversals or divide-and-conquer recurrences) can be executed concurrently if read-write place sets do not alias.

## Requirements
- **PARALLEL-01**: Extend MIR with `Terminator::Fork { left, right, join }`.
- **PARALLEL-02**: Implement subtree/knot independence analysis (`ReadWriteSet`, `sets_are_independent`, `find_parallel_knot_pairs`) in `src/mir/supercompiler/independence.rs`.
- **PARALLEL-03**: Add `residualize_process_tree_parallel`.
- **PARALLEL-04**: Lower `Fork` in native backends with runtime shim `__numlang_fork_join`.
- **PARALLEL-05**: Expose opt-in `--parallel-residualize` CLI flag.

## Key Deliverables
- `src/mir/supercompiler/independence.rs`
- `src/mir/supercompiler/residualize.rs`
- `tests/supercompiler_phase36_tests.rs`

## Verification
- `cargo test --test supercompiler_phase36_tests` passes 5/5 tests green.
- Independent recursion trees emit valid `Terminator::Fork` and execute in parallel.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
