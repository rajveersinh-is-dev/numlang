# Phase 62: Whole-Program Cross-Function Recurrence Closing — Plan

> **Phase**: 62
> **Status**: Planned
> **Traceability**: Master Plan Part VI, Requirements XFUNC-01..05
> **Milestone**: World's Fastest General Supercompiler (Phases 59–66)

## Objective
Collapse mutual recursion across function boundaries via whole-program cross-function companion matrix construction and $N$-way recurrence solving.

## Root Cause / Motivation
Currently, NumLang's recurrence solver operates strictly within intra-procedural loop bodies. Mutually recursive functions (e.g. `even(n) = odd(n-1)`, `odd(n) = even(n-1)`, or multi-function state machines) exhibit periodic cyclic behavior across function calls that the intra-procedural driver cannot fold into closed form. Closing cross-function mutual recurrences closes the key capability gap against Sørensen's positive supercompilation.

## Requirements
- **XFUNC-01**: In `drive.rs`, detect inter-procedural call cycles in the process tree where function $A$ calls $B$ which calls $A$.
- **XFUNC-02**: Extract joint transition matrices representing the composite state transformation across the mutual call cycle.
- **XFUNC-03**: Interface with `solve_nway_recurrence` in `src/mir/supercompiler/recurrence.rs` to construct an $N \times N$ companion matrix for the coupled system.
- **XFUNC-04**: Emit residualized matrix exponentiation or closed-form expressions that compute the mutual recursion result in $O(\log N)$ or $O(1)$.
- **XFUNC-05**: Verification in `tests/mutual_recursion_collapse_tests.rs`: verify closed-form collapse of `even/odd`, 2-level mutual recursions, and Hofstadter-style linear systems.

## Key Deliverables
- `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/recurrence.rs`
- Test suite: `tests/mutual_recursion_collapse_tests.rs`

## Verification Gate
- `cargo test --test mutual_recursion_collapse_tests` passes 100%.
- Verified mutual recursive functions specialize to non-recursive closed-form code.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
