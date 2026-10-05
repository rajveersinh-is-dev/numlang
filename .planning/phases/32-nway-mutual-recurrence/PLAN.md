# Phase 32: N-Way Mutual Recurrence Solver — Plan

> **Phase**: 32
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 32
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Add `NWayLinearSystem`, detect mutually recursive systems using integer Cramer's rule and Bareiss fraction-free Gaussian elimination ($N \le 8$), and solve via binary matrix exponentiation.

## Root Cause / Motivation
Real-world algorithms often exhibit coupled mutual recurrences (e.g., mutually recursive state transitions or multi-variable linear recurrences) that scalar induction cannot contract.

## Requirements
- **MUTUAL-01**: Implement `NWayLinearSystem` and `detect_nway_linear_system` using integer Cramer's rule and fraction-free Gaussian elimination ($N \le 8$).
- **MUTUAL-02**: Implement binary matrix exponentiation `mat_pow_nxn` for $N \times N$ state transition matrices.
- **MUTUAL-03**: Implement `solve_nway_recurrence` emitting `__nway_recurrence_i` intrinsics and exact closed forms.
- **MUTUAL-04**: Integrate into `SupercompilerDriver` loop recurrence pipeline.

## Key Deliverables
- `src/mir/supercompiler/recurrence.rs`
- `tests/supercompiler_phase32_tests.rs`

## Verification
- `cargo test --test supercompiler_phase32_tests` passes 100%.
- Multi-variable coupled recurrences contract to $O(\log N)$ binary exponentiation.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
