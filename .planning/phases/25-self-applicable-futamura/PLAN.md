# Phase 25: Genuine Self-Applicable Specializer & 2nd and 3rd Futamura Projections — Plan

> **Phase**: 25
> **Status**: Completed
> **Traceability**: Master Plan §6, Requirements FUTA-01..FUTA-04
> **Milestone**: Remediation & Frontier (Phases 20–28)

## Objective
Resolve the self-application impossibility by authoring `MinSpec.nl` in NumLang itself and verifying the 1st, 2nd, and 3rd Futamura projections.

## Root Cause / Motivation
Prior Futamura claims relied on mock Rust scripts rather than genuine self-application of a partial evaluator written in the target language specializing itself.

## Requirements
- **FUTA-01**: Implement a self-contained, self-applicable partial evaluator `MinSpec.nl` in NumLang source code (`src/stdlib/minspec.nl`).
- **FUTA-02**: Verify 1st Futamura projection: $\text{MinSpec}(\text{interp}, \text{prog}) \to \text{prog\_compiled}$.
- **FUTA-03**: Verify 2nd Futamura projection: $\text{MinSpec}(\text{MinSpec}, \text{interp}) \to \text{compiler}$.
- **FUTA-04**: Verify 3rd Futamura projection: $\text{MinSpec}(\text{MinSpec}, \text{MinSpec}) \to \text{cogen}$ and prove $\text{cogen}(\text{interp}) \equiv \text{compiler}$.

## Key Deliverables
- `src/stdlib/minspec.nl`
- `tests/true_futamura_projections_tests.rs`

## Verification
- `cargo test --test true_futamura_projections_tests` passes 100%.
- Verified 1st, 2nd, and 3rd Futamura projections with structural output differences.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
