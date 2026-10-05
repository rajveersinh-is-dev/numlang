# Phase 60: Nonlinear Polynomial Recurrence Solver — Plan

> **Phase**: 60
> **Status**: Planned (In Progress)
> **Traceability**: Master Plan Part VI, Requirements POLYREC-01..05
> **Milestone**: World's Fastest General Supercompiler (Phases 59–66)

## Objective
Extend `src/mir/supercompiler/recurrence.rs` and `drive.rs` with a nonlinear polynomial and geometric recurrence solver, enabling closed-form collapse ($O(N) \to O(1)$ / $O(\log N)$) for quadratic recurrences, geometric progressions, and exponential fixed-base powers.

## Root Cause / Motivation
Currently, NumLang's closed-form solver in `recurrence.rs` handles linear affine recurrences ($T(n) = a T(n-1) + b$) and companion matrix systems ($x_{n} = M x_{n-1}$). However, nonlinear recurrences, geometric series ($S(n) = \sum r^k$), exponential powers ($k^n$), and degree-2 polynomial sums ($S(n) = \sum k^2 = \frac{n(n+1)(2n+1)}{6}$) fail to collapse, reverting to unrolled loops or knot-tying iterations. GHC Supercompiler and Hermit cannot solve these in closed form.

## Requirements
- **POLYREC-01**: Implement quadratic recurrence detection and closed-form polynomial sum formulas: $\sum_{k=1}^n k = \frac{n(n+1)}{2}$, $\sum_{k=1}^n k^2 = \frac{n(n+1)(2n+1)}{6}$, $\sum_{k=1}^n k^3 = \left(\frac{n(n+1)}{2}\right)^2$ in `src/mir/supercompiler/recurrence.rs`.
- **POLYREC-02**: Implement geometric series solver: recognize $acc_{k} = acc_{k-1} + c \cdot r^k$ with invariant ratio $r \neq 1$, generating closed form $c \cdot \frac{r^{n+1} - 1}{r - 1} + init$ using symbolic binary exponentiation (`sym_pow`).
- **POLYREC-03**: Implement exponential power recurrence solver: recognize $acc_{k} = acc_{k-1} \cdot k$ where $k$ is loop-invariant, generating $init \cdot k^n$ in $O(\log n)$ operations.
- **POLYREC-04**: Integrate polynomial/geometric solver into `try_solve_loop_recurrence` in `src/mir/supercompiler/drive.rs` so that when linear matrix analysis fails, the nonlinear polynomial and geometric detectors fire automatically.
- **POLYREC-05**: Verification in `tests/polynomial_recurrence_tests.rs`: verify closed-form collapse and exact numerical equivalence for square pyramid sums, geometric sums, and exponential power loops.

## Key Deliverables
- `src/mir/supercompiler/recurrence.rs`: Nonlinear polynomial, exponential, and geometric recurrence solvers.
- `src/mir/supercompiler/drive.rs`: Integration into `try_solve_loop_recurrence`.
- `tests/polynomial_recurrence_tests.rs`: Unit and integration test suite.

## Verification Gate
- `cargo test --test polynomial_recurrence_tests` passes 100%.
- Verified `loops_collapsed` metric increments for polynomial and geometric loops.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
