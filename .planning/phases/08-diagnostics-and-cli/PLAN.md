# Phase 8: Production Diagnostic Polish & Explain CLI — Plan

> **Phase**: 08
> **Status**: Completed
> **Traceability**: Master Plan Part I, Requirements DIAG-01..04
> **Milestone**: Production Compiler Baseline (Phases 1–9)

## Objective
Integrate `miette` span-highlighted reporting across all syntax and semantic errors, adding actionable suggestions and an interactive `--explain <CODE>` CLI.

## Requirements
- **DIAG-01**: Catalog standardized compiler error codes `E0001` through `E0025` in `src/diagnostic.rs`.
- **DIAG-02**: Integrate `miette` source snippet rendering with colored underlines, line numbers, and primary/secondary labels.
- **DIAG-03**: Attach exact source `Span`s across lexer tokens, AST nodes, and type-checker errors.
- **DIAG-04**: Implement CLI command `numlang --explain <ERROR_CODE>` printing long-form explanations with problematic and fixed code examples.

## Key Deliverables
- `src/diagnostic.rs`, `src/typecheck/checker.rs`, `src/main.rs`
- Test suite: `tests/diagnostics_tests.rs`

## Verification
- Verified console rendering of highlighted spans on deliberate syntax/type errors.
- Verified `--explain E0001` outputs documentation and examples.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
