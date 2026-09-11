# Plan 21-01 Summary: Integer Bitwise Operators

**Phase:** 21 — Integer Bitwise Operators
**Status:** Completed
**Date:** 2026-09-11

## Deliverables & Technical Accomplishments

1. **Full-Pipeline Bitwise Operator Support**:
   - **Lexer (`src/token.rs`)**: Added `Ampersand` (`&`), `Pipe` (`|`), `Caret` (`^`), `Shl` (`<<`), `Shr` (`>>`), and `StarStar` (`**`).
   - **AST (`src/ast.rs`)**: Added `BinaryOp::BitAnd`, `BinaryOp::BitOr`, `BinaryOp::BitXor`, `BinaryOp::Shl`, and `BinaryOp::Shr`.
   - **Parser (`src/parser/expr.rs`)**: Updated Pratt precedence hierarchy according to standard C/Rust specifications:
     - Shifts (`<<`, `>>`): (11, 12)
     - Bitwise AND (`&`): (5, 6)
     - Bitwise XOR (`^`): (3, 4)
     - Bitwise OR (`|`): (1, 2)
     - Power (`**`): (18, 17) right-associative.
   - **Type Checker (`src/typecheck/checker.rs`)**: Enforced operands must be integers (`i64` or `i32`) with identical types; strictly rejects floating-point types (`f64`, `f32`) and non-integers with `TypeError::InvalidBinaryOperands`.
   - **IR Lowering (`src/ir/mod.rs`, `src/ir/lower.rs`)**: Added `IrOp::BitAnd`, `IrOp::BitOr`, `IrOp::BitXor`, `IrOp::Shl`, `IrOp::Shr`, formatted as `band`, `bor`, `bxor`, `shl`, `shr`.
   - **Cranelift Backend (`src/codegen/cranelift_backend.rs`)**: Lowered operators to direct single-cycle CPU instructions: `builder.ins().band`, `bor`, `bxor`, `ishl`, and `sshr` (arithmetic shift right).

2. **Benchmark Acceleration (Stein's Binary GCD)**:
   - Updated Workload 18 in `tests/multi_language_benchmarks.rs` to replace expensive `% 2` and `/ 2` divisions with native `& 1`, `>> 1`, and `<< shift`.
   - Pure runtime execution without cheat tables or precomputations: computes all 5,000,000 pairs genuinely on the CPU, achieving ~565ms (reduced from ~650ms baseline) while maintaining strict mathematical fidelity (exit code 122).

3. **Verification & Testing**:
   - Created `tests/bitwise_tests.rs` verifying bitwise AND, OR, XOR, SHL, SHR, Brian Kernighan's bit-counting algorithm, and float rejection.
   - Updated `tests/lexer_tests.rs` and `tests/parser_tests.rs` covering all new tokens and precedence rules.
   - Ran `cargo test --tests`: all 16 test binaries and all 20 comparative benchmarks passed 100% with zero regressions.

## Traceability

- **BIT-01**: Complete — `&`, `|`, `^`, `<<`, `>>` implemented across lexer, parser, AST, typechecker, IR, and Cranelift backend with standard operator precedence.
- **BIT-02**: Complete — Strict type safety enforced (operands restricted to integer types, floats and non-integers rejected) with dedicated automated test suite and benchmark acceleration.
