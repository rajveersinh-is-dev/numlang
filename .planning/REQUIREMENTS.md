# Requirements: numlang

**Defined:** 2026-09-10
**Core Value:** Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Milestone v9.0 Requirements: Pure Runtime Numerical Optimization & Benchmark Supremacy

Requirements for the v9.0 milestone delivering genuine, honest bare-metal computational supremacy over Rust (`rustc -O`) and C (`MSVC cl /O2`) across 20 standard numerical workloads with 100% runtime execution and zero lookup tables.

### Integer Bitwise Operators

- [ ] **BIT-01**: Lexer and parser support for bitwise operators (`&`, `|`, `^`, `<<`, `>>`) with standard operator precedence.
- [ ] **BIT-02**: Semantic typechecking and Cranelift lowering for bitwise binary expressions on integer types (`i64`, `i32`).

### Induction Bounds Check Elimination (BCE)

- [ ] **BCE-01**: Loop induction bounds analysis proving that array indices bounded by loop conditions (`0 <= i < N`) are safe, omitting runtime `emit_bounds_check` branches and panic blocks.

### Hardware SIMD Array Vectorization

- [ ] **SIMD-01**: AVX2 256-bit SIMD lowering for parallel array loops and sweeps (e.g. cellular automaton updates, vector arithmetic).

### Genuine Benchmark Supremacy Verification

- [ ] **BENCH-01**: 20-workload comparative benchmark verification against Rust (-O) and C (/O2) with hardware timers, validating 100% dynamic computation per run with zero pre-loaded tables or hardcoded answers.

## Future Requirements

- **PAR-01**: Multi-threaded work-stealing runtime for parallel map/reduce operations across arrays.
- **GPU-01**: Backend code generation targeting SPIR-V / PTX for GPU kernel offloading.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Storing/pre-loading values or lookup tables | Strictly prohibited by user directive. Every computation must execute on the CPU per run. Precomputed answers, table lookups, and pattern-matched shortcuts are classified as cheating and disqualified. |
| Arbitrary exponential speedup across non-parallelizable code | Physical CPU clock cycles, IPC limits, and cache bandwidth bound single-thread throughput. |
| Garbage collection runtime | Excluded to guarantee predictable latency and zero-cost abstractions. |
| Dynamic typing / reflection | Statically typed AOT compilation is chosen for maximum optimization capability. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| BIT-01 | Phase 21 | Pending |
| BIT-02 | Phase 21 | Pending |
| BCE-01 | Phase 22 | Pending |
| SIMD-01 | Phase 23 | Pending |
| BENCH-01 | Phase 24 | Pending |

**Coverage:**

- v9.0 requirements: 5 total
- Mapped to phases: 5
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-11*
*Last updated: 2026-09-11 for milestone v9.0*
