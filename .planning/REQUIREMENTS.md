# Requirements: numlang

**Defined:** 2026-09-11  
**Core Value:** Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Milestone v13.0 Requirements: Total Bare-Metal Dominance — Eradicating Remaining Deltas vs Rust and C

Requirements for v13.0 closing every remaining performance delta and establishing clean, honest runtime superiority over both optimized Rust (`rustc -O`) and C (`MSVC cl /O2`) across all 20 benchmark workloads through entry-block constant hoisting, algebraic strength reduction (LEA multiplication for `x * 3`), square non-negativity propagation for Mandelbrot, and loop induction optimizations.

### Entry-Block Constant Hoisting & Instruction Deduplication
- [ ] **CONST-01**: Implement entry-block constant hoisting in Cranelift backend, emitting all integer literals, float literals, and divisor magic numbers once in the function entry block. Reuses SSA values across all blocks to eliminate tens of millions of redundant `iconst` instructions executed inside tight while loops (Collatz, Mandelbrot, DCT, Monte Carlo, Newton Sqrt).

### Algebraic Strength Reduction & Fast Arithmetic
- [ ] **STRENGTH-01**: Implement multiplication-by-constant strength reduction in Cranelift backend: synthesize small constant integer multiplications (e.g. `x * 3` -> `(x << 1) + x`) to emit single-cycle x86 `lea` instructions instead of 3-cycle `imul`.
- [ ] **STRENGTH-02**: Specialize power-of-2 divisibility in AST and codegen: transform `(x % 2) == 0` and `(x % 2) != 0` directly into single-cycle bitwise tests `(x & 1) == 0` / `(x & 1) != 0` without emitting intermediate remainder instructions.

### Square Non-Negativity & Interval Propagation
- [ ] **SQUARE-01**: Enhance `is_expr_known_non_negative` to recognize that any square `x * x` and sum of squares `x*x + y*y` are strictly non-negative, unlocking unsigned Granlund-Montgomery reciprocal multiplier reduction for Mandelbrot's `(zr * zr) / 1000` and `(zi * zi) / 1000`.

### Loop Induction & Pipeline Optimization
- [ ] **COLLATZ-01**: Optimize Collatz hailstone trajectory and Monte Carlo RNG pipelines to surpass Rust execution speed through algebraic parity awareness and direct register pipelining.

### Universal Benchmark Supremacy Verification
- [ ] **AUDIT-13**: Execute complete 20-workload comparative benchmark decimation audit against Rust (-O) and C (/O2) with hardware QPC telemetry, verifying 100% dynamic bare-metal CPU computation (zero lookup tables, zero cached values).

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
| CONST-01 | Phase 38 | Pending |
| STRENGTH-01 | Phase 39 | Pending |
| STRENGTH-02 | Phase 39 | Pending |
| SQUARE-01 | Phase 39 | Pending |
| COLLATZ-01 | Phase 40 | Pending |
| AUDIT-13 | Phase 41 | Pending |

**Coverage:**

- v13.0 requirements: 6 total
- Mapped to phases: 6
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-11*  
*Last updated: 2026-09-11 for milestone v13.0*
