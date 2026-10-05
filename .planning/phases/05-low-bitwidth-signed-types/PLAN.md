# Phase 5: Low-Bitwidth Signed Types (i8 and i16) — Plan

> **Phase**: 05
> **Status**: Completed
> **Traceability**: Master Plan Part I, Requirements TYPE-01..04
> **Milestone**: Production Compiler Baseline (Phases 1–9)

## Objective
Complete the integer type matrix by introducing signed 8-bit (`i8`) and signed 16-bit (`i16`) integers alongside the existing unsigned and 32/64-bit types.

## Requirements
- **TYPE-01**: Lexer recognition for `i8` and `i16` literal suffixes in `TypedIntLiteral`.
- **TYPE-02**: Type system representation `Type::I8` and `Type::I16` with correct size (1, 2 bytes) and alignment.
- **TYPE-03**: Semantic checking for sign-extension, truncating casts, and arithmetic bounds checking.
- **TYPE-04**: Cranelift backend mapping to native `types::I8` and `types::I16` registers.

## Key Deliverables
- `src/token.rs`, `src/typecheck/types.rs`, `src/typecheck/checker.rs`, `src/codegen/cranelift/`
- Test suite: `tests/unsigned_type_tests.rs` (extended for signed low-bitwidth)

## Verification
- Verified wrapping arithmetic, sign-extension, and boundary values (-128..127, -32768..32767).
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
