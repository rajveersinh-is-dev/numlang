# Milestones

## v2.0 Benchmark Supremacy (Shipped: 2026-09-10)

**Phases completed:** 3 phases (Phases 6, 7, 8), 5 plans

**Key accomplishments:**
- **Host CPU SIMD & FMA Vector Engine**: Dynamic detection of `avx2`, `fma`, `sse4.2`, `bmi1`, `bmi2` via CPUID, activating hardware FMA instructions (`vfmadd213sd`/`vfmadd231sd`) in Cranelift backend.
- **Bounds Check Elimination (BCE)**: Static induction variable range analysis and monotonicity tracking in semantic analysis, omitting runtime array boundary comparisons and panic branch traps in verified loops.
- **Full & 4x Loop Unrolling**: Complete unrolling of small fixed loops (`N <= 16`) to straight-line instructions with zero branches, and 4x unrolled pipelining for general induction loops yielding a 75% reduction in branch penalties.
- **8-Way Multi-Accumulator Pipelining**: 8 independent accumulator pipelines for vector `dot`, `sum`, and `vec_add` coupled with depth-3 binary reduction trees to saturate dual x86 FMA execution ports.
- **Verified Benchmark Victory**:
  - **Math Loop Accumulator (10M iters)**: numlang (**50.38ms**) outperforms both Rust (**51.32ms**) and MSVC C (**57.40ms**).
  - **Hardware SIMD Vector Dot (10M iters)**: numlang (**70.63ms**) is **2.15x faster** than optimized MSVC C (**151.74ms**).
  - **Dense Matrix-Vector Multiplication (1M iters)**: Operates at direct parity with native C and Rust.
  - **100% Bit-for-Bit Equivalence**: Identical mathematical results and exit codes across all workloads.

---

## v1.0 v1.0 (Shipped: 2026-09-10)

**Phases completed:** 5 phases, 10 plans, 0 tasks

**Key accomplishments:**

- Bootstrapped the numlang Rust compiler crate with strong numeric AST definitions and a high-performance Logos tokenizer.
- Implemented the complete syntax parsing engine combining a top-down operator precedence (Pratt) parser for mathematical expressions with recursive descent for functions and control flow.
- Built the CLI driver and rich diagnostic error reporting engine for numlang, verifying token and AST inspection via automated integration tests.
- 2026-09-10
- 2026-09-10
- 2026-09-10

---
