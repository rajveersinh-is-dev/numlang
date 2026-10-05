# Phase 23: Real Polyhedral Loop & Stencil Deforestation — Plan

> **Phase**: 23
> **Status**: Completed
> **Traceability**: Master Plan §4, Requirements POLY-01..POLY-03
> **Milestone**: Remediation & Frontier (Phases 20–28)

## Objective
Replace forward variable substitution with true polyhedral affine loop and stencil fusion, contracting intermediate array memory buffers to $O(1)$ scalar temporaries or sliding windows.

## Root Cause / Motivation
Array-intensive numerical workloads previously suffered heavy memory allocations and cache churn due to separate producer and consumer loop nests that could be legally fused into streaming stencils.

## Requirements
- **POLY-01**: Extract affine iteration domain polyhedra $\{ \vec{i} \mid A \vec{i} + \vec{b} \ge \vec{0} \}$ and access matrices in `src/mir/supercompiler/polyhedral.rs`.
- **POLY-02**: Compute data dependence distance vectors between producer loops and consumer loops.
- **POLY-03**: Perform legal affine loop fusion and contract intermediate array buffers to $O(1)$ scalar temporaries or sliding windows.

## Key Deliverables
- `src/mir/supercompiler/polyhedral.rs`
- `tests/polyhedral_stencil_tests.rs`

## Verification
- `cargo test --test polyhedral_stencil_tests` passes 100%.
- Multi-pass array stencils contract intermediate buffers to $O(1)$ scalar temporaries.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
