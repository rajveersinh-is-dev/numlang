# Phase 34: Full Higher-Order Closure Driving — Plan

> **Phase**: 34
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 34
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Add `SymTerm::ClosureVal`, track closure allocations and function pointers, drive through indirect calls, bind captured variables, and deforest higher-order loops.

## Root Cause / Motivation
Higher-order functions with captured environments were blocked during symbolic driving, requiring conservative residualization of indirect calls and losing deforestation opportunities.

## Requirements
- **HODRIVE-01**: Add `SymTerm::ClosureVal(String, Vec<SymTermId>, Type)` for symbolic closures.
- **HODRIVE-02**: Track `Rvalue::ClosureAlloc` and `Rvalue::FnPtr` in `drive_statement`.
- **HODRIVE-03**: Drive through `Terminator::IndirectCall` in `drive_node` via `resolve_closure_function` and `try_drive_closure_call`.
- **HODRIVE-04**: Bind captured variables and call arguments into callee initial state with interval refinement propagation.
- **HODRIVE-05**: Fold closure invocations and deforest higher-order loops.

## Key Deliverables
- `src/mir/supercompiler/term.rs`
- `src/mir/supercompiler/drive.rs`
- `tests/supercompiler_phase34_tests.rs`

## Verification
- `cargo test --test supercompiler_phase34_tests` passes 6/6 tests green.
- Higher-order pipelines drive symbolically through indirect calls to direct calls.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
