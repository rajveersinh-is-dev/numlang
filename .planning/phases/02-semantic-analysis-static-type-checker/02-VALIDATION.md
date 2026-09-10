# Phase 2: Semantic Analysis & Static Type Checker - Validation Plan

**Domain:** Unit testing, semantic test harnesses, static type checking assertions
**Confidence:** HIGH

## Verification Commands

Execute the following automated commands:
```bash
cargo test --test typecheck_tests
cargo test
```

## Validation Scenarios

1. **Primitive Numerical Types & Inference:**
   - Verify unannotated integers infer as `i64`.
   - Verify unannotated floats infer as `f64`.
   - Verify explicit type annotations (`i32`, `i64`, `f32`, `f64`, `bool`) are enforced.
2. **Rejection of Implicit Type Coercion:**
   - Verify `f64 + i64` produces `TypeMismatch` compile error.
   - Verify `i32 + i64` produces `TypeMismatch` compile error.
3. **Lexical Scoping & Shadowing:**
   - Verify nested blocks can shadow outer variables.
   - Verify identifiers out-of-scope produce `UndeclaredVariable` error.
4. **Immutability Enforcement:**
   - Verify reassignment to an immutable variable produces `CannotMutateImmutable` error.
   - Verify reassignment to `let mut` variable succeeds when types match.
5. **Function Signature Validation:**
   - Verify calling a function with incorrect number of arguments produces `ArityMismatch`.
   - Verify calling a function with incorrect argument types produces `TypeMismatch`.
   - Verify return statement type matches function signature.
   - Verify undeclared function calls produce `UndeclaredFunction`.
6. **Control Flow Validation:**
   - Verify non-boolean conditions in `if` and `while` are rejected.
