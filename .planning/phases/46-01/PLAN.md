# Phase 46: Post-Review Loose-Ends Cleanup — Plan

> **Phase**: 46
> **Status**: Completed
> **Traceability**: Master Plan §15, Requirements CLEAN-01..CLEAN-05
> **Milestone**: Adversarial Remediation & System Soundness (Phases 41–47)

## Objective
Convert remaining codegen panics to `Err(CodegenError::BackendError)`, relocate AST recurrence lowering to `src/opt/recursion.rs`, purge dead OS allocation fields, generalize `DUMP_CLIF`, upgrade benchmark warmup iterations ($\ge 5$), and re-scan the entire tree for dead code.

## Root Cause / Motivation
Post-review inspection following Phases 41–45 identified lingering `panic!()` calls in Cranelift AST statements and expressions, misplaced AST recurrence lowering in the backend, unused OS allocator identifiers left after Win32 decoupling, hardcoded target checks in debug dumpers, and insufficient warmup cycles in multi-language benchmark harnesses.

## Requirements
- **CLEAN-01**: In `src/codegen/cranelift/ast_stmt.rs` and `ast_expr.rs`, convert all remaining 5 `panic!()` sites to structured `Err(CodegenError::BackendError(...))`.
- **CLEAN-02**: Relocate `try_lower_binary_recurrence_tree` from `src/codegen/cranelift/mod.rs` to `src/opt/recursion.rs` alongside tail-call optimization.
- **CLEAN-03**: Remove dead `local_alloc_id` and `os_malloc_id` fields and unused system allocator declarations from `CraneliftCompiler`.
- **CLEAN-04**: Generalize `DUMP_CLIF` in `src/codegen/cranelift/mod.rs` to dump all functions when empty or target an arbitrary function name string.
- **CLEAN-05**: Harden `tests/multi_language_benchmarks.rs` to enforce $\ge 5$ discarded warmup iterations before timing measurement.

## Key Deliverables
- `src/codegen/cranelift/ast_stmt.rs`
- `src/codegen/cranelift/ast_expr.rs`
- `src/codegen/cranelift/mod.rs`
- `src/codegen/cranelift/intrinsics.rs`
- `src/opt/recursion.rs`
- `tests/multi_language_benchmarks.rs`

## Verification
- `cargo test --lib` passes 100%.
- Zero `panic!()` occurrences remain in `src/codegen/cranelift/`.
- Zero `#[allow(dead_code)]` suppressions across active `src/`.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
