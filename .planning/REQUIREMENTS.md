# Requirements: numlang

**Defined:** 2026-09-11
**Core Value:** Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Milestone v10.0 Requirements: Compiler Hardening & Universal Benchmark Supremacy

Requirements for the v10.0 milestone delivering genuine bare-metal computational supremacy over Rust (`rustc -O`) and C (`MSVC cl /O2`) across all 20 standard numerical workloads by eliminating backend code generation bottlenecks, SROA dynamic indexing degradation, hardware division stalls, and loop overhead.

### SROA Optimization & Contiguous Indexing

- [x] **SROA-01**: Restrict SROA register promotion to purely statically indexed arrays; dynamically indexed arrays remain contiguous stack slots (`Storage::Array`).
- [x] **SROA-02**: Eliminate `O(len)` CMOV select trees on dynamic array reads and writes, lowering dynamic lookups to single-cycle indexed memory operations (`mov rax, [rsp + rdi*8]`), accelerating N-Queens and search loops.

### High-Throughput Modulo & Division Strength Reduction

- [x] **DIV-01**: Non-negative / unsigned fast path for power-of-two modulo: lower `x % (1 << k)` to single-cycle `band_imm` (`x & ((1 << k) - 1)`), eliminating 6-8 instruction sign-bias arithmetic in RNG and Monte Carlo simulation.
- [x] **DIV-02**: Loop-invariant divisor strength reduction: detect invariant divisors in loops and optimize lowering to reciprocal multiplication or fast unsigned paths.

### Loop Optimization & Dynamic BCE

- [x] **LOOP-01**: While loop code generation optimization: eliminate condition re-evaluation overhead and prune dead merge blocks in Cranelift lowering.
- [x] **LOOP-02**: Expand BCE (`src/opt/bce.rs`) interval range analysis to handle binary search midpoint formulas `mid = (low + high) / 2` when `0 <= low <= high < len`, marking `arr[mid]` as `is_safe = true` and eliminating bounds checks.

### Universal Benchmark Supremacy Verification

- [ ] **BENCH-01**: Execute complete 20-workload comparative benchmark suite against Rust (-O) and C (/O2) with hardware QPC timers.
- [ ] **BENCH-02**: Verify 100% computed runtime values (0 lookup tables, 0 precomputed answer injections, 0 cached values) and document decisive performance superiority across all 20 workloads.

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
| SROA-01 | Phase 25 | Complete |
| SROA-02 | Phase 25 | Complete |
| DIV-01 | Phase 26 | Complete |
| DIV-02 | Phase 26 | Complete |
| LOOP-01 | Phase 27 | Complete |
| LOOP-02 | Phase 27 | Complete |
| BENCH-01 | Phase 28 | Pending |
| BENCH-02 | Phase 28 | Pending |

**Coverage:**

- v10.0 requirements: 8 total
- Mapped to phases: 8
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-11*
*Last updated: 2026-09-11 for milestone v10.0*
