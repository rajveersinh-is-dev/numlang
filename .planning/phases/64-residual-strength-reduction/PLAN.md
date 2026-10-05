# Phase 64: Strength Reduction in Residual After Loop Collapse — Plan

> **Phase**: 64
> **Status**: Complete
> **Traceability**: Master Plan Part VI, Requirements STRENGTH-01..05
> **Milestone**: World's Fastest General Supercompiler (Phases 59–66)

## Objective
Implement a post-collapse MIR strength reduction pass and Strassen block matrix multiply for $N \ge 4$, optimizing closed-form residuals to match or surpass GCC/LLVM `-O3` code quality.

## Root Cause / Motivation
When the supercompiler collapses an $O(N)$ loop into a closed-form formula (e.g. polynomial sum or matrix exponentiation), the emitted residual MIR frequently contains multiplications by constant powers of two, constant divisions, or nested matrix multiplication loops. Performing strength reduction on the residual MIR ensures that algebraic elegance translates directly into minimal CPU instruction cycles.

## Requirements
- **STRENGTH-01**: Implement `src/mir/supercompiler/strength_reduce.rs` scanning residual basic blocks for strength reduction opportunities.
- **STRENGTH-02**: Power-of-2 multiplication reduction: replace `mul(x, 2^k)` with `shl(x, k)`.
- **STRENGTH-03**: Near-power-of-2 reduction: replace `mul(x, 2^a ± 2^b)` with shift and add/sub sequences.
- **STRENGTH-04**: Power-of-2 division reduction: replace `div(x, 2^k)` with arithmetic right shifts (`shr`).
- **STRENGTH-05**: Integrate Strassen block recursion for $N \times N$ matrix exponentiation where $N \ge 4$ in `recurrence.rs`.

## Key Deliverables
- `src/mir/supercompiler/strength_reduce.rs`, `src/mir/supercompiler/recurrence.rs`
- Test suite: `tests/strength_reduce_tests.rs`

## Verification Gate
- `cargo test --test strength_reduce_tests` passes 100%.
- Verified residual MIR replaces expensive multiplications with bit shifts.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
