# Phase 9: Built-in Standard Library (std) — Plan

> **Phase**: 09
> **Status**: Completed
> **Traceability**: Master Plan Part I, Requirements STD-01..05
> **Milestone**: Production Compiler Baseline (Phases 1–9)

## Objective
Provide built-in mathematical intrinsics, bitwise utilities, and type conversion primitives directly linked into Cranelift code generation.

## Requirements
- **STD-01**: Mathematical intrinsics: `sqrt`, `abs`, `min`, `max`.
- **STD-02**: Bit manipulation primitives: `popcnt`, `clz`, `ctz`, `bswap`.
- **STD-03**: Floating-point conversions: `i64_to_f64`, `f64_to_i64`.
- **STD-04**: Cranelift intrinsic lowering emitting dedicated machine instructions (e.g. `clz`, `popcnt`, `fsqrt`).
- **STD-05**: Linkage to runtime math functions (`sin`, `cos`, `tan`, `ln`, `exp`) for transcendental operations.

## Key Deliverables
- `src/typecheck/checker.rs`, `src/codegen/cranelift/`
- Test suite: `tests/stdlib_tests.rs`

## Verification
- Verified numerical accuracy across all bitwise and floating-point intrinsics.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
