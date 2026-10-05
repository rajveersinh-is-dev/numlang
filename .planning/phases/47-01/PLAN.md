# Phase 47: Hardening, Clippy Purity & Safety Audit — Plan

> **Phase**: 47
> **Status**: Completed
> **Traceability**: Master Plan §16, Requirements HARDEN-01..HARDEN-05
> **Milestone**: Adversarial Remediation & System Soundness (Phases 41–47)

## Objective
Eliminate all `clippy::needless_return` warnings across codegen match arms, fix platform portability symbol inspection, eliminate repetitive `.unwrap()` calls in MIR lowering with safe monadic helpers, and enrich `CodegenError` with domain variants.

## Root Cause / Motivation
Codebase inspection following Phase 46 revealed needless return statements flagged by Clippy, platform portability test failures due to modular symbol relocation into `intrinsics.rs`, unsafe unwrap patterns in MIR lower block management, and over-reliance on generic string-based `BackendError` variants.

## Requirements
- **HARDEN-01**: Eliminate all `clippy::needless_return` instances in `src/codegen/cranelift/ast_expr.rs` and `ast_stmt.rs`.
- **HARDEN-02**: Update `tests/platform_portability_tests.rs` to inspect both `mod.rs` and `intrinsics.rs` with `Linkage::Import`.
- **HARDEN-03**: Eliminate 20+ repetitive `.unwrap()` calls in `src/mir/lower.rs` with safe monadic helpers `push_stmt`, `current_block_id`, `set_terminator`, and `current_terminator`.
- **HARDEN-04**: Extend `CodegenError` in `src/codegen/cranelift/abi.rs` with domain-specific variants (`VariableNotFound`, `InvalidArrayTarget`, `MissingLayout`, `FieldNotFound`, `UnsupportedOp`).
- **HARDEN-05**: Ensure whole-workspace `cargo clippy --all-targets -- -D warnings` and `cargo check --tests` pass with zero warnings.

## Key Deliverables
- `src/codegen/cranelift/ast_expr.rs`
- `src/codegen/cranelift/ast_stmt.rs`
- `src/codegen/cranelift/intrinsics.rs`
- `src/codegen/cranelift/abi.rs`
- `src/mir/lower.rs`
- `tests/platform_portability_tests.rs`

## Verification
- `cargo test --test platform_portability_tests` passes 6/6 tests.
- `cargo test --lib` passes 100%.
- Zero `.unwrap()` calls in `src/mir/lower.rs`.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
