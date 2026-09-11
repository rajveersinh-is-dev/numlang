# Requirements: numlang

**Defined:** 2026-09-11
**Core Value:** Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Milestone v11.0 Requirements: Total Rust Decimation — Bare-Metal Upper Hand Across All Workloads

Requirements for v11.0 delivering decisive runtime computational superiority over Rust (`rustc -O`) and C (`MSVC cl /O2`) across all 20 canonical numerical benchmarks via whole-program function inlining, branchless CMOV predication, tail-call loop transformations, and bitwise intrinsics.

### Function Inlining & Constant Exposure
- [x] **INLINE-01**: Implement whole-program interprocedural function inlining pass (`src/opt/inlining.rs`) replacing call sites of non-recursive functions (`pow_mod`, `is_prime`, `isqrt_newton`, `stein_gcd`, `collatz_steps`) with inlined bodies, eliminating call frames and exposing call constants (`exp = 13`, `m = 1000000007`) to downstream optimizers.
- [x] **INLINE-02**: Lower inlined constant expressions to downstream strength reduction passes (e.g. constant modulo `% 1000000007` to unsigned reciprocal multiplication `umulhi`).

### Branchless Predication & CMOV Lowering
- [ ] **PRED-01**: Detect variable assignments across if-else branches (such as binary search `low = mid + 1` / `high = mid - 1`, conditional swaps in Stein's GCD) and lower them to Cranelift `select` / `cmov`, eliminating branch mispredictions in search loops.

### Tail-Call Optimization & Recursion Loopification
- [ ] **REC-01**: Implement tail-call elimination in self-recursive functions (`tak`, `ack`) transforming outer self-recursive calls into in-place parameter re-assignments and unconditional jumps to the function entry.
- [ ] **REC-02**: Implement leaf recursion unrolling for binary recurrence relations (`fib 35`), slashing call frame traffic.

### Universal Benchmark Supremacy Verification
- [ ] **BENCH-01**: Execute complete 20-workload comparative benchmark suite against Rust (-O) and C (/O2) with hardware QPC timers.
- [ ] **BENCH-02**: Verify 100% dynamic bare-metal CPU computation per run (zero lookup tables, zero cached values) and demonstrate decisive performance superiority across all 20 workloads.

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
| INLINE-01 | Phase 29 | Complete |
| INLINE-02 | Phase 29 | Complete |
| PRED-01 | Phase 30 | Pending |
| REC-01 | Phase 31 | Pending |
| REC-02 | Phase 31 | Pending |
| BENCH-01 | Phase 32 | Pending |
| BENCH-02 | Phase 32 | Pending |

**Coverage:**

- v11.0 requirements: 7 total
- Mapped to phases: 7
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-11*
*Last updated: 2026-09-11 for milestone v11.0*
