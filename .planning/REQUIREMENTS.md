# Requirements: numlang

**Defined:** 2026-09-10
**Core Value:** Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Milestone v2.0 Requirements: Benchmark Supremacy

Requirements for the v2.0 performance-focused milestone targeting decisive benchmark victories.

### Target CPU Specialization & SIMD Vectorization

- [x] **SIMD-01**: The Cranelift code generation backend enables host x86_64 CPU target features (`has_avx2`, `has_fma`, `has_sse42`, `has_bmi2`) for machine-level vector instruction selection.
- [x] **SIMD-02**: Vector math intrinsics (`dot`, `vec_add`, `sum`) emit hardware-accelerated FMA (Fused Multiply-Add) and vectorized accumulator instructions.
- [x] **SIMD-03**: Built-in array and vector operations support AVX2 chunking for batch floating-point and integer processing.

### Loop Optimization & Bounds Check Elimination (BCE)

- [x] **OPT-01**: Static induction variable and range analysis determines loop bounds and variable monotonicity at compile time.
- [x] **OPT-02**: Bounds Check Elimination (BCE) eliminates runtime boundary check branches in loops when index variables are statically proven within `0 <= i < len`.
- [x] **OPT-03**: Loop unrolling optimization pass unrolls fixed-size array iterations and vector operations (4x/8x) to maximize instruction pipelining and eliminate branch penalties.

### Benchmark Suite & Advantage Verification

- [ ] **BENCH-02**: Extended comparative benchmark suite implements comprehensive workloads: recursive Fibonacci (`fib(35)`), tight math accumulator (10M iters), SIMD vector dot product (10M iters), and matrix-vector multiplication.
- [ ] **BENCH-03**: Automated benchmark verification records and reports statistically significant execution speed advantages for `numlang` against optimized C (`cl.exe /O2`) and Rust (`rustc -O`).

## v3 Requirements (Future)

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
| SIMD-01 | Phase 6 | Complete |
| SIMD-02 | Phase 6 | Complete |
| SIMD-03 | Phase 6 | Complete |
| OPT-01 | Phase 7 | Complete |
| OPT-02 | Phase 7 | Complete |
| OPT-03 | Phase 7 | Complete |
| BENCH-02 | Phase 8 | Pending |
| BENCH-03 | Phase 8 | Pending |

**Coverage:**

- v2 requirements: 8 total
- Mapped to phases: 8
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-10*
*Last updated: 2026-09-10 for milestone v2.0*
