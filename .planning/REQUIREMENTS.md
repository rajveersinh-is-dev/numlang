# Requirements: numlang

**Defined:** 2026-09-11  
**Core Value:** Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Milestone v12.0 Requirements: Universal Bare-Metal Transcendence — Outperforming Rust and C Across All Workloads

Requirements for v12.0 closing every remaining performance delta and establishing clean, honest runtime superiority over both optimized Rust (`rustc -O`) and C (`MSVC cl /O2`) across all 20 benchmark workloads through hardware bit manipulation intrinsics, bounded while-loop unrolling, branchless scalar select lowering, and leaf recursion expansion.

### Hardware Bit Manipulation & Loop Recognition
- [ ] **BIT-01**: Implement native hardware bit-manipulation intrinsics (`ctz`, `clz`, `popcnt`, `rotl`, `rotr`) across compiler pipeline (lexer, parser, typechecker, Cranelift lowering) mapping directly to x86-64 single-cycle machine instructions (`tzcnt`/`bsf`, `lzcnt`/`bsr`, `popcnt`, `rol`, `ror`).
- [ ] **BIT-02**: Detect trailing-zero while loops (`while (u & 1) == 0 { u = u >> 1; }`) in Cranelift backend and lower to single-cycle hardware shift `u = u >> ctz(u)`, slashing Stein's Binary GCD from 484 ms to < 250 ms and Rule 110 from 107 µs to < 40 µs.

### Bounded While-Loop Unrolling & Exponentiation Expansion
- [ ] **UNROLL-01**: Implement bounded while-loop unrolling in `src/opt/loop_unrolling.rs` for loops with small compile-time known trip counts.
- [ ] **UNROLL-02**: Lower constant-exponent `pow_mod` loops into straight-line square-and-multiply multiplication and reciprocal modulo chains, slashing Modular Exponentiation from 44.36 ms to < 20 ms.

### Branchless Scalar Select Predication
- [ ] **SELECT-01**: Generalize branchless select predication in `src/codegen/cranelift_backend.rs` to handle general scalar variable updates in if-else statements (such as `if (n & 1) == 0 { n = n >> 1; } else { n = 3 * n + 1; }`), slashing Collatz Hailstone from 14.52 ms to < 8 ms.

### Leaf Recursion Base-Case Expansion
- [ ] **REC-03**: Implement leaf recursion base-case unrolling and dual expansion for binary recurrences (`fib 35`), slashing `fib(35)` runtime from 28.08 ms to < 19 ms.

### Universal Benchmark Supremacy Verification
- [ ] **BENCH-03**: Execute the complete 20-workload comparative benchmark suite against Rust (-O) and C (/O2) with hardware QPC timers, verifying 100% dynamic bare-metal CPU computation (zero lookup tables, zero cached values) and demonstrating decisive performance superiority across the suite.

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
| BIT-01 | Phase 33 | Pending |
| BIT-02 | Phase 33 | Pending |
| UNROLL-01 | Phase 34 | Pending |
| UNROLL-02 | Phase 34 | Pending |
| SELECT-01 | Phase 35 | Pending |
| REC-03 | Phase 36 | Pending |
| BENCH-03 | Phase 37 | Pending |

**Coverage:**

- v12.0 requirements: 7 total
- Mapped to phases: 7
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-11*  
*Last updated: 2026-09-11 for milestone v12.0*
