# Phase 31: Formal Termination Certificate & Order-3 Symbolic Recurrence — Plan

> **Phase**: 31
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 31
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Add machine-readable `TerminationWitness` recording whistle firings and header cutoffs, implement `--emit-termination-proof` CLI, and extend linear recurrence solver to symbolic trip counts for order-3 recurrences.

## Root Cause / Motivation
Supercompiler termination must be auditable and mathematically verifiable for external formal tools, and order-3 linear recurrences (e.g. Tribonacci) require symbolic closed-form matrix lowering.

## Requirements
- **TERM-01**: Add machine-readable `TerminationWitness` capturing homeomorphic embedding whistle firings and process tree cutoffs.
- **TERM-02**: Implement CLI `--emit-termination-proof` emitting structured JSON certificates.
- **TERM-03**: Extend linear recurrence solver to symbolic trip counts for order-3 recurrences via `__order3_recurrence` intrinsic and native codegen lowering.

## Key Deliverables
- `src/mir/supercompiler/whistle.rs`
- `src/mir/supercompiler/recurrence.rs`
- `tests/supercompiler_phase31_tests.rs`

## Verification
- `cargo test --test supercompiler_phase31_tests` passes 100%.
- JSON termination certificates successfully validate process tree well-foundedness.
- Order-3 recurrence solver computes exact values matching reference implementation.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
