# Phase 3: Core Control Flow (For Loops, Continue, Loop) — Plan

> **Phase**: 03
> **Status**: Completed
> **Traceability**: Master Plan Part I, Requirements CF-01..05
> **Milestone**: Production Compiler Baseline (Phases 1–9)

## Objective
Expand NumLang's control flow syntax from simple `while` loops to include `for .. in` range loops, `continue`, and infinite `loop` blocks, providing ergonomic control structures for systems programming.

## Root Cause / Motivation
Baseline NumLang only supported structured `while` loops. Implementing complex algorithms (such as sieve, matrix traversal, and iterative numerical solvers) required manual index variable management and error-prone loop bookkeeping.

## Requirements
- **CF-01**: Lexer tokens: `For`, `In`, `Continue`, `Loop`, `DotDot` (`..`), `DotDotEq` (`..=`).
- **CF-02**: AST nodes and parsing in `src/ast.rs` and `src/parser/stmt.rs` for `Stmt::For`, `Stmt::Continue`, `Stmt::Loop`.
- **CF-03**: Type checker validation for range bounds, loop nesting depth tracking, and rejection of `continue`/`break` outside loop contexts.
- **CF-04**: IR lowering desugaring `Stmt::For` into canonical while loops with induction increments.
- **CF-05**: Cranelift backend support for `continue` jumps targeting active loop header basic blocks.

## Key Deliverables
- `src/token.rs`, `src/ast.rs`, `src/parser/stmt.rs`, `src/typecheck/checker.rs`, `src/ir/lower.rs`, `src/codegen/cranelift/`
- Test suite: `tests/control_flow_tests.rs`

## Verification
- `cargo test --test control_flow_tests` passes 100%.
- Verified inclusive/exclusive range iterations, nested loops, and `continue` skipping.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
