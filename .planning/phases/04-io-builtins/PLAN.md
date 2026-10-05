# Phase 4: Standard I/O Built-ins (Print and Println) — Plan

> **Phase**: 04
> **Status**: Completed
> **Traceability**: Master Plan Part I, Requirements IO-01..04
> **Milestone**: Production Compiler Baseline (Phases 1–9)

## Objective
Implement built-in `print` and `println` intrinsics supporting formatted console output for integers, floats, booleans, and string constants without requiring external libc dependencies.

## Requirements
- **IO-01**: Lexer support for `StringLiteral` tokens with escape sequences (`\n`, `\t`, `\\`, `\"`).
- **IO-02**: AST expression nodes for string literals and intrinsic call dispatch for `print` and `println`.
- **IO-03**: Type checker validation allowing primitive types (int, float, bool) and string literals as arguments.
- **IO-04**: Codegen runtime emission using native OS output handles: Win32 `WriteFile` on `STD_OUTPUT_HANDLE` on Windows, and POSIX `write(1)` syscalls on Unix.

## Key Deliverables
- `src/token.rs`, `src/ast.rs`, `src/parser/expr.rs`, `src/typecheck/checker.rs`, `src/codegen/cranelift/`
- Test suite: `tests/io_tests.rs`

## Verification
- `cargo test --test io_tests` passes.
- Verified stdout capture for `"Hello World"`, negative integers, booleans, and floats.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
