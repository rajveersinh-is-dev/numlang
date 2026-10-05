# Phase 65: CPS Transformation of the Driving Loop (Infinite Stack Safety) — Plan

> **Phase**: 65
> **Status**: Complete
> **Traceability**: Master Plan Part VI, Requirements CPS-01..05
> **Milestone**: World's Fastest General Supercompiler (Phases 59–66)

## Objective
Convert recursive driving into a work-queue-based trampoline to support unbounded recursion depth ($> 1,000$) and enable multi-threaded parallel driving across functions.

## Root Cause / Motivation
In the current implementation, `drive_node` uses native Rust call-stack recursion. For programs with deep call chains or complex AST nesting (>128 recursion depth), driving is truncated or aborts with stack overflow safeguards. Converting driving into an explicit Continuation-Passing Style (CPS) trampoline using a heap-allocated work queue eliminates stack limits entirely and allows distributing driving work across worker threads via lock-free work-stealing queues.

## Requirements
- **CPS-01**: Define explicit driving task structures `enum DriveTask { ProcessNode(ProcessNodeId), HandleTransition(...) }`.
- **CPS-02**: Replace recursive `drive_node` calls with a work-queue trampoline (`VecDeque<DriveTask>`).
- **CPS-03**: Reconstruct ancestor chain paths directly from the process-tree DAG rather than relying on call-stack activation frames.
- **CPS-04**: Enable parallel work-stealing driving using `crossbeam-deque` or scoped threads across independent process branches.
- **CPS-05**: Verification in `tests/deep_recursion_safety_tests.rs`: verify programs with recursion depths > 1,000 drive cleanly without stack overflow, matching single-threaded outputs.

## Key Deliverables
- `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/parallel.rs`
- Test suite: `tests/deep_recursion_safety_tests.rs`

## Verification Gate
- Deep recursion programs compile cleanly without stack overflow.
- Multi-threaded driving outputs identical residual MIR to sequential driving.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
