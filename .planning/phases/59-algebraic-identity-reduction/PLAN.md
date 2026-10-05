# Phase 59: Algebraic Identity Reduction in Term Interning — Plan

> **Phase**: 59
> **Status**: In Progress
> **Traceability**: Master Plan §5, Requirements ALG-01..05
> **Closes gap vs**: GHC supercompiler, Stahl's SC, GCC/Clang float constant folding

## Objective
Eliminate redundant symbolic terms during interning through zero/one identity rules, idempotence, inverse cancellation, float constant folding and identity reductions, double negation elimination, and canonical commutativity ordering at intern time in `TermInterner`.

## Root Cause of Gap
Every pass of the homeomorphic embedding whistle (`is_embedded`, `state_embeds`) and every MSG anti-unification comparison operates on `SymTermId` pairs. If `x + 0`, `0 + x`, and `x` intern to different IDs, the whistle can fail to detect embedding and the MSG generates spurious fresh variables, causing unnecessary generalization. This directly prevents loops from being recognized as closed-form recurrences and causes missed supercompilation opportunities. Furthermore, floating-point operations currently intern as opaque binary terms without constant folding or identity reductions (`x * 1.0`, `x + 0.0`).

## Requirements
- **ALG-01: Canonical Commutative Ordering**:
  For commutative binary operators (`Add`, `Mul`, `BitAnd`, `BitOr`, `BitXor`, `Eq`, `Ne`), reorder operands canonically (non-constant on left, constant on right; if both are non-constant, order by `SymTermId`). This ensures `a + b` and `b + a` intern to identical `SymTermId`s.
- **ALG-02: Structural Zero/One/Identity Reductions**:
  Implement zero/one and identity rules in `TermInterner::intern_binary` for:
  - Add/Sub: `x + 0 = x`, `x - 0 = x`, `x - x = 0`, `0 - x = -x` (with type-safe literals).
  - Mul/Div: `x * 1 = x`, `x * 0 = 0`, `x * -1 = -x`, `x / 1 = x`, `x / x = 1` ($x \neq 0$), `0 / x = 0` ($x \neq 0$).
  - Bitwise: `x & x = x`, `x & 0 = 0`, `x | x = x`, `x | 0 = x`, `x ^ x = 0`, `x ^ 0 = x`.
  - Shift/Mod: `x << 0 = x`, `x >> 0 = x`, `0 << x = 0`, `0 >> x = 0`, `x % 1 = 0`, `x % x = 0`, `0 % x = 0`.
  - Logical: `x && true = x`, `x && false = false`, `x || false = x`, `x || true = true`.
  - Comparison on identical terms: `x == x` -> `true`, `x != x` -> `false`, `x <= x` -> `true`, `x >= x` -> `true`, `x < x` -> `false`, `x > x` -> `false`.
  - Reassociation of constant chains: `(x + c1) + c2 = x + (c1 + c2)`, `(x - c1) - c2 = x - (c1 + c2)`, `(x * c1) * c2 = x * (c1 * c2)`.
- **ALG-03: Unary Inversion and Double Negation Elimination**:
  In `TermInterner::intern_unary`:
  - `¬¬x = x` (double boolean negation).
  - `--x = x` (double arithmetic negation).
  - Constant evaluation of unary expressions on int, float, and bool.
- **ALG-04: Floating-Point Constant Folding & Identities**:
  - Full constant folding for float arithmetic (`+`, `-`, `*`, `/`) and comparisons (`<`, `<=`, `>`, `>=`, `==`, `!=`).
  - Float identities: `x + 0.0 = x`, `x - 0.0 = x`, `x * 1.0 = x`, `x / 1.0 = x`.
  - Proper type preservation: returning `ConstFloat(0.0, ty)` or `ConstInt(0, ty)` matching expression `ty`.
- **ALG-05: Verification & Zero Regressions**:
  - Comprehensive test suite `tests/algebraic_reduction_tests.rs` with 30+ algebraic identities tested.
  - Zero regressions across existing test suites (`cargo test`).
  - Clippy purity: `cargo clippy --all-targets -- -D warnings` reports 0 errors and 0 warnings.

## Verification
- `cargo test --test algebraic_reduction_tests` passes 100%.
- Full test suite passes without regressions.
- `cargo clippy --all-targets -- -D warnings` reports 0 warnings.
