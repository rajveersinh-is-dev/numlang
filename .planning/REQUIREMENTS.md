# Requirements: numlang

**Defined:** 2026-09-10
**Core Value:** Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Milestone v3.0 Requirements: Total Rust Decimation

Requirements for the v3.0 performance-focused milestone targeting decisive benchmark victories over Rust (`rustc -O`) across all 4 benchmark workloads.

### Recursive Call Optimization

- [ ] **REC-01**: Recursive call unrolling optimization pass expands self-recursive calls by depth 1-2 (`fib(n-1) -> fib(n-2) + fib(n-3)`), eliminating 50%+ of recursive call frames and overhead.

### Scalar Replacement of Aggregates (SROA) & SSA Register Promotion

- [ ] **SROA-01**: Scalar Replacement of Aggregates (SROA) promotes small fixed array elements (`N <= 16`) into Cranelift SSA variables, eliminating stack memory allocations, loads, and stores.
- [ ] **SROA-02**: Array element reads `arr[c]` and mutations `arr[c] = v` for promoted arrays lower directly to SSA register reads and variable definitions (`builder.use_var`, `builder.def_var`).
- [ ] **SROA-03**: Vector operations (`dot`, `vec_add`, `sum`) operating on promoted arrays execute directly using SSA register values without memory round-trips.

### Total Benchmark Supremacy Verification

- [ ] **VICTORY-01**: Automated benchmark verification proves `numlang` achieves statistically significant speedup over `rustc -O` across all 4 benchmark workloads (Fibonacci, Math Loop Accumulator, SIMD Dot, and Matrix-Vector Multiplication) while preserving 100% bit-for-bit mathematical output equivalence.

## Future Requirements

- **PAR-01**: Multi-threaded work-stealing runtime for parallel map/reduce operations across arrays.
- **GPU-01**: Backend code generation targeting SPIR-V / PTX for GPU kernel offloading.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Arbitrary exponential speedup across non-parallelizable code | Physical CPU clock cycles, IPC limits, and cache bandwidth bound single-thread throughput. |
| Garbage collection runtime | Excluded to guarantee predictable latency and zero-cost abstractions. |
| Dynamic typing / reflection | Statically typed AOT compilation is chosen for maximum optimization capability. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| REC-01 | Phase 9 | Pending |
| SROA-01 | Phase 10 | Pending |
| SROA-02 | Phase 10 | Pending |
| SROA-03 | Phase 10 | Pending |
| VICTORY-01 | Phase 11 | Pending |

**Coverage:**

- v3 requirements: 5 total
- Mapped to phases: 5
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-10*
*Last updated: 2026-09-10 for milestone v3.0*
