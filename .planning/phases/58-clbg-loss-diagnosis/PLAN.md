# Phase 58: CLBG Loss Diagnosis & Fix Plan — Plan

> **Phase**: 58
> **Status**: Completed
> **Traceability**: Master Plan Part V, Requirements CLBG-01..05
> **Milestone**: Real-World Dominance & CLBG Optimization

## Objective
Diagnose and resolve performance regressions on Computer Language Benchmarks Game (CLBG) workloads, specifically preventing the driver from partially unrolling loops it cannot solve, crushing instruction caches.

## Requirements
- **CLBG-01**: Early cutoff for branching loop bodies: when `try_solve_accumulator_loop` fails due to conditional bodies (e.g. `pidigits` alternating sum), set forced knot cutoff to `header_visits >= 1` instead of unrolling 12 iterations.
- **CLBG-02**: Small constant loop unrolling guard: when loop bound is constant but body contains non-pure calls (`binary_trees`), prevent partial unrolling bloat.
- **CLBG-03**: MSG generalization for knot transfers: generalize differing variable states so knots can tie naturally before forced cutoffs.
- **CLBG-04**: Code size bloat guard: if residual basic block count exceeds $3\times$ source block count without closed-form collapse, reject specialization and preserve baseline MIR.
- **CLBG-05**: Verify CLBG suite performance (`spectral_norm`, `nbody`, `fannkuch_redux`, `mandelbrot`, `pidigits`, `binary_trees`) beats or matches baseline C/MSVC /O2.

## Key Deliverables
- `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/generalize.rs`, `tests/clbg_correctness_tests.rs`

## Verification
- `pidigits` residual blocks reduced from 57 to 7; runs faster than baseline C.
- `tests/clbg_correctness_tests.rs` passes 100%.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
