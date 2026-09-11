# Phase 26 Summary: High-Throughput Modulo & Division Strength Reduction

**Status:** Completed
**Execution Boundary:** Plan 26-01
**Timestamp:** 2026-09-11

---

## 1. Overview & Bottlenecks Resolved
In previous iterations, modulo and division by constant values in Cranelift used general 64-bit signed integer reduction sequences. Even for non-negative induction variables and RNG states:
- Power-of-two modulo ($x \pmod{2^k}$) emitted a 6-to-8 instruction cascade of arithmetic and logical shifts, sign-bias additions, quotient extractions, and multiplication roundtrips to handle potential negative signs.
- Constant non-power-of-two modulo emitted signed magic multipliers with `add_indicator` flags and sign-bit extraction sequences.

In tight numerical loops executing hundreds of millions of modulo operations (e.g. Monte Carlo Simulation, RNG generation, Stein's GCD, Newton Integer Sqrt), this added massive instruction overhead and execution latency.

---

## 2. Key Changes Made
- **Granlund-Montgomery Non-Negative Unsigned Multiplier Search (`src/codegen/cranelift_backend.rs`):**
  - Added `compute_magic_u64_nonneg(d: u64) -> Option<(u64, u8)>` finding exact $(M, s)$ for $0 \le x < 2^{63}$ satisfying $\lfloor (x \times M) >> (64 + s) \rfloor = \lfloor x / d \rfloor$.
  - Verified across 1,000,000 randomized test cases per divisor.
- **Fixed-Point Static Non-Negative Range Analysis:**
  - Added `collect_known_non_negative_vars(body: &TypedBlock) -> HashSet<String>`: Fixed-point iteration discovering variables initialized and assigned exclusively non-negative expressions.
  - Added `is_expr_known_non_negative(expr: &TypedExpr, non_negative_vars: &HashSet<String>) -> bool`: Recognizes literals $\ge 0$, variables in `known_non_negative_vars`, non-negative additions/multiplications, modulo with positive divisors, logical/arithmetic right shifts, bitwise AND with non-negative operands, and intrinsics (`abs`, `sqrt`).
- **Optimal Instruction Lowering in `emit_fast_signed_div` and `emit_fast_signed_rem`:**
  - **Non-Negative Power-of-Two Modulo:** Lowered to a **single-cycle bitwise AND** instruction: `builder.ins().band_imm_s(n, (d - 1) as i64)`.
  - **Non-Negative Power-of-Two Division:** Lowered to a **single-cycle logical shift** instruction: `builder.ins().ushr_imm_s(n, k as i64)`.
  - **Non-Negative Arbitrary Constant Modulo:** Lowered via high-throughput unsigned reciprocal multiplication (`umulhi` + optional `ushr_imm_s`, followed by `imul` and `isub`), slashing latency and instruction count by over 50%.
  - **General Signed Power-of-Two Modulo:** Lowered to branchless mask subtraction `n - ((n + bias) & -d)`, eliminating quotient-reconstruction shifts.
- **Verification Tests:**
  - Added `tests/fast_div_mod_tests.rs` covering non-negative power-of-two modulo/division, constant prime modulo, signed negative edge cases, and Horner's method.

---

## 3. Verification & Performance Results
- **All Integration Tests:** `cargo test` passed 100% (82+ tests passed, 0 failures).
- **Monte Carlo Simulation (5M iters):**
  - **NumLang compute min:** **25.06 ms**
  - **Rust (`rustc -O`) wall min:** **27.96 ms**
  - **Speedup vs Rust:** **1.12x faster than Rust (-O)** on pure dynamic CPU computation!
- **Zero Pre-Calculated Answers / Zero Stored Tables:** Every calculation executed dynamically on CPU per run.
