# Phase 2 Self-Audit: Soundness of Recurrence Collapse and the Pipeline

Date: 2026-10-10  
Auditor / Orchestrator: Team Peer Review  
Status: **DONE**  

---

## 1. Hostile Re-read: The 5 Weakest Points & Attack Experiments

1. **Weak Point 1: Intermediate 64-bit Integer Overflow in Closed Forms (F10)**
   - *Attack*: Evaluated degree-2 recurrence collapse with large trip count $k \ge 3 \times 10^9$. Naive computation of $k \cdot (k - 1) / 2$ evaluates $(3 \cdot 10^9) \cdot (3 \cdot 10^9 - 1) \approx 9 \times 10^{18}$, which exceeds `i64::MAX` ($9.22 \times 10^{18}$) and wraps into negative values before division by 2, yielding unsound results.
   - *Result*: Test `test_recurrence_large_k_overflow_soundness` reproduced the overflow when using standard naive formulas.
   - *Resolution*: Implemented `build_triangular_term` in `src/mir/supercompiler/generalize.rs` and safe closed-form expansions in `src/mir/supercompiler/recurrence.rs`. Employs parity-halving: if $k$ is even, compute $(k / 2) \cdot (k - 1)$; if $k$ is odd, compute $k \cdot ((k - 1) / 2)$. If the difference constant $d_2$ is even, factor $d_2 / 2$ directly into the product to avoid division entirely.

2. **Weak Point 2: Stale Specialization Disk Cache Invalidation (F10)**
   - *Attack*: Compiled a function with version $V_1$, modified optimization flags or compiler binary to $V_2$, and queried the on-disk cache in `.numlang_cache/`.
   - *Result*: The cache key only contained the function MIR hash, leading to stale cache hits across incompatible compiler versions or flag profiles. Additionally, corrupt disk cache files would panic during deserialization.
   - *Resolution*: Updated `CacheKey` in `src/mir/supercompiler/cache.rs` to include `compiler_version: String` (`env!("CARGO_PKG_VERSION")`) and `optimization_flags: String`. Implemented graceful error recovery on corrupted disk cache entries by logging a warning and recording a cache miss rather than panicking. Verified via `tests/supercompiler_phase37_tests.rs` (8/8 tests passed).

3. **Weak Point 3: Cranelift MIR Codegen Struct Lowering & Differential Failures**
   - *Attack*: Added `test_differential_examples_directory` in `tests/differential_correctness_tests.rs` comparing Cranelift MIR codegen execution against the reference AST interpreter on all programs in `examples/`.
   - *Result*: `examples/point.nl` crashed with access violation `0xC0000005` under Cranelift MIR execution. Structs fell through to null-pointer assignments in `mir_emit.rs`, and field projections loaded 64-bit integers from offset 0 regardless of field layout.
   - *Resolution*: Implemented complete struct layout computation, signature flattening (unpacking struct arguments into leaf ABI values), stack-slot allocation for `Rvalue::Struct`, field offset projection in `get_place_value`, and store support for `Statement::Assign` with `Projection::Field`.

4. **Weak Point 4: String Literal Lowering & Print Dispatch in Cranelift MIR**
   - *Attack*: Ran `examples/hello.nl` and `examples/closure.nl` through Cranelift MIR execution.
   - *Result*: String literals returned null pointers and were dispatched to `print_i64` instead of `print_str`.
   - *Resolution*: In `mir_emit.rs`, allocated stack slots for string literal byte contents, tracked string slice lengths in `string_lengths`, and lowered `print`/`println` statements for strings to invoke `print_str_id` passing `[addr, len]`.

5. **Weak Point 5: SMT Validator & Language Specification Documentation (F12)**
   - *Attack*: Inspected documentation against actual compiler implementation in `src/mir/supercompiler/validate.rs`.
   - *Result*: `LANGUAGE.md` lacked specifications for wrapping arithmetic semantics, the stratified memory model, Reynolds defunctionalization, and the in-tree QF_BV / CDCL SAT solver.
   - *Resolution*: Authored Sections 9–13 of `LANGUAGE.md` and detailed `validate.rs` mechanics in `docs/FORMAL_VERIFICATION.md`.

---

## 2. Verification Evidence

- `cargo test --test polynomial_recurrence_tests`: **PASSED** (includes `test_recurrence_large_k_overflow_soundness`)
- `cargo test --test supercompiler_phase37_tests`: **PASSED** (8/8 tests, including cache invalidation and corruption fallback)
- `cargo test --test differential_correctness_tests`: **PASSED** (all 3 tests, including `test_differential_examples_directory`)
- `python scripts/claims_lint.py`: **PASSED** (100% compliance)
- `cargo clippy --all-targets -- -D warnings`: **PASSED** (0 errors, 0 warnings)

---

## 3. Assumptions, Guesses & Unverified Areas

- High-degree polynomial sum closed forms ($d \ge 3$) use parity/divisibility factor ordering; extremely high step counts approaching $2^{63} / 6$ remain bounded by 64-bit integer range, where NumLang's two's-complement wrapping arithmetic applies.

---

## 4. Confidence Assessment

- **Recurrence Overflow Soundness**: **HIGH**. Parity-halving avoids intermediate overflow for all valid triangular sums fitting in `i64`.
- **Cache Invalidation & Robustness**: **HIGH**. Version and flags are strictly keyed; corrupted files degrade safely to misses.
- **Cranelift MIR Codegen & Differential Equivalence**: **HIGH**. Mechanically verified across all example files against the reference interpreter.
