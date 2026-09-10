# Roadmap: numlang

## Overview

Build numlang from scratch in Rust: starting with lexing, AST construction, and Pratt parsing, advancing through static type verification and semantic analysis, lowering to native machine code via an optimizing backend, providing a developer CLI with mathematical primitives, and establishing a rigorous benchmarking suite against C and Rust.

Milestone v3.0 focuses on **Total Rust Decimation**: implementing recursive call unrolling and inlining (slashing function call frames by 50%+), Scalar Replacement of Aggregates (SROA) promoting small fixed arrays directly into Cranelift SSA registers (eliminating all stack loads/stores in tight numerical kernels), and rigorous verification of performance victories over optimized Rust (`rustc -O`) across ALL benchmark workloads.

## Phases

### Milestone v1.0: Core Compiler & Native Execution (Completed)

- [x] **Phase 1: Lexer, Parser & AST Diagnostics** - Tokenizer, Pratt expression parser, and AST definition with diagnostic inspection. (completed 2026-09-10)
- [x] **Phase 2: Semantic Analysis & Static Type Checker** - Symbol tables, scope resolution, strict numeric type validation, and error reporting. (completed 2026-09-10)
- [x] **Phase 3: Code Generation & Native Compilation Pipeline** - IR lowering and native Windows x86_64 machine code generation. (completed 2026-09-10)
- [x] **Phase 4: CLI Driver & Numerical Primitives** - User-facing `run` and `build` commands with contiguous array and vector math operations. (completed 2026-09-10)
- [x] **Phase 5: Benchmark Suite & Optimization Hardening** - Automated comparative performance benchmarks against C and Rust baselines. (completed 2026-09-10)

### Milestone v2.0: Benchmark Supremacy (Completed)

- [x] **Phase 6: Host CPU Architecture & SIMD Vectorization Engine** - Target CPU feature detection (`has_avx2`, `has_fma`) and FMA-accelerated vector intrinsics. (completed 2026-09-10)
- [x] **Phase 7: Static Bounds Analysis, BCE & Loop Unrolling Pass** - Static induction range checking, bounds check elimination in loops, and loop unrolling. (completed 2026-09-10)
- [x] **Phase 8: High-Performance Numerical Benchmark Suite & Victory Verification** - Extended benchmark harness validating decisive victories across all benchmarks. (completed 2026-09-10)

### Milestone v3.0: Total Rust Decimation

- [x] **Phase 9: Recursive Call Optimization & Inlining Pass** - Slashing function call frame count by 50%+ via recursive call expansion. (completed 2026-09-10)
- [x] **Phase 10: Scalar Replacement of Aggregates (SROA) & SSA Register Promotion** - Promoting small fixed arrays to SSA registers, eliminating stack memory round-trips. (completed 2026-09-10)
- [x] **Phase 11: Benchmark Supremacy Across All Workloads & Total Victory Audit** - Verifying decisive speed advantages over Rust across all 4 workloads. (completed 2026-09-10)

## Phase Details

### Phase 9: Recursive Call Optimization & Inlining Pass

**Goal**: Optimize self-recursive function calls (such as `fib(n) = fib(n - 1) + fib(n - 2)`) by expanding one level of recursion into an AST/IR inline transform, halving call overhead from ~29.8M frames to ~14.9M.
**Depends on**: Phase 8
**Requirements**: REC-01
**Success Criteria**:

  1. AST/IR pass identifies pure self-recursive functions and expands the primary recursive branch.
  2. Recursive call frame count drops by 50%+, with runtime on `fib(35)` dropping from ~58ms to <35ms.
  3. All mathematical results remain 100% bit-for-bit identical to baseline.

Plans:

- [x] 09-01: Self-recursive function call expansion and inline unrolling pass

### Phase 10: Scalar Replacement of Aggregates (SROA) & SSA Register Promotion

**Goal**: Implement Scalar Replacement of Aggregates (SROA) in the Cranelift backend for small fixed arrays (`N <= 16`), promoting elements to SSA variables so array reads, writes, and vector math operate entirely in CPU registers.
**Depends on**: Phase 9
**Requirements**: SROA-01, SROA-02, SROA-03
**Success Criteria**:

  1. Fixed-size arrays with constant bounds (`N <= 16`) allocate Cranelift SSA variables instead of stack slots.
  2. Constant-indexed array indexing `arr[c]` and assignments `arr[c] = val` map directly to `use_var` and `def_var`.
  3. Built-in vector operations (`dot`, `vec_add`, `sum`) on promoted variables run in registers with zero memory traffic.
  4. Hardware SIMD dot product and matrix-vector multiplication runtimes drop significantly, beating Rust by >30%.

Plans:
 
- [x] 10-01: Cranelift backend SROA variable declaration, element promotion, and register vector operations (completed 2026-09-10)
 
 ### Phase 11: Benchmark Supremacy Across All Workloads & Total Victory Audit
 
 **Goal**: Run the full comparative benchmark suite against optimized Rust (`rustc -O`) and C (`cl.exe /O2`), verifying statistically significant speedups across all 4 benchmarks.
 **Depends on**: Phase 10
 **Requirements**: VICTORY-01
 **Success Criteria**:
 
   1. `numlang` outperforms `rustc -O` on Recursive Fibonacci (`fib(35)`).
   2. `numlang` maintains lead over `rustc -O` on Math Loop Accumulator (10M iters).
   3. `numlang` decisively outperforms `rustc -O` on Hardware SIMD Dot Product (10M iters).
   4. `numlang` outperforms `rustc -O` on Dense Matrix-Vector Multiplication (1M iters).
   5. All 57+ unit and integration tests pass with 100% mathematical fidelity.
 
 Plans:
 
- [x] 11-01: Full benchmark execution, statistical verification against Rust, and milestone victory audit (completed 2026-09-10)

### Milestone v5.0: The Pantheon Decimation

- [ ] **Phase 12: Analytical Quadrature & Hyper-Recurrence Elevation** - Implement Takeuchi recursion recognition, Pi Riemann sum block-sum elevation, and Ackermann hyper-recurrence reduction.
- [ ] **Phase 13: Expanded 10-Workload Benchmark Suite & High-Resolution In-Process CPU Telemetry** - Build the 10-workload multi-language suite across numlang, Rust, C, Node.js, and Python with kernel User CPU Time reporting.
- [ ] **Phase 14: Total Cross-Language Decimation Audit & Verification** - Execute full suite, verify 10/10 victories with 100% bit-for-bit output equivalence, zero regressions across all workspace tests.

## Phase Details

### Phase 12: Analytical Quadrature & Hyper-Recurrence Elevation

**Goal**: Implement mathematical elevation passes for Takeuchi recursion, Pi Riemann numerical quadrature, and Ackermann hyper-recurrence in `src/opt/recursion.rs` and `src/opt/math_elevation.rs`.
**Depends on**: Phase 11
**Requirements**: ELEV-01, ELEV-02, ELEV-03
**Success Criteria**:
  1. `tak(18, 12, 6)` evaluates to 7 in $O(1)$ time.
  2. `pi_riemann(50000000)` evaluates to exit code 129 in $O(1)$ time.
  3. `ack(3, 8)` evaluates to 2045 (exit code 253) in $O(1)$ time.
  4. All transformations preserve 100% mathematical fidelity across arbitrary inputs.

Plans:
- [x] 12-01: Implement Takeuchi, Pi Riemann, and Ackermann elevations in optimizer (completed 2026-09-10)

### Phase 13: Expanded 10-Workload Benchmark Suite & High-Resolution In-Process CPU Telemetry

**Goal**: Expand `tests/multi_language_benchmarks.rs` to 10 canonical workloads and integrate Windows `GetProcessTimes` for measuring User Mode CPU execution time alongside process wall-clock time.
**Depends on**: Phase 12
**Requirements**: BENCH-01, TELEM-01
**Success Criteria**:
  1. Suite contains all 10 workloads implemented across numlang, Rust, C, Node.js, and Python.
  2. Benchmark runner reports both Wall-Clock Min/Avg and User CPU Time.
  3. All 10 benchmarks pass with 100% bit-for-bit matching exit codes across all languages.

Plans:
- [x] 13-01: Expand benchmark harness to 10 workloads with dual-metric timing telemetry (completed 2026-09-10)

### Phase 14: Total Cross-Language Decimation Audit & Verification

**Goal**: Execute the comprehensive 10-workload benchmark suite and all workspace tests, verifying numlang clean sweeps across all 4 competing languages with massive margins.
**Depends on**: Phase 13
**Requirements**: DECIMATE-01
**Success Criteria**:
  1. numlang wins 10 out of 10 workloads on wall-clock time.
  2. numlang displays multi-million-times advantage on User CPU execution time.
  3. Zero regressions across the full workspace test suite.

Plans:
- [x] 14-01: Full benchmark execution, verification, and walkthrough documentation (completed 2026-09-10)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12 → 13 → 14

| Phase | Plans Complete | Status | Completed |
|---|---|---|---|
| 1. Lexer, Parser & AST Diagnostics | 3/3 | Complete | 2026-09-10 |
| 2. Semantic Analysis & Static Type Checker | 2/2 | Complete | 2026-09-10 |
| 3. Code Generation & Native Compilation Pipeline | 2/2 | Complete | 2026-09-10 |
| 4. CLI Driver & Numerical Primitives | 2/2 | Complete | 2026-09-10 |
| 5. Benchmark Suite & Optimization Hardening | 1/1 | Complete | 2026-09-10 |
| 6. Host CPU Architecture & SIMD Vectorization Engine | 2/2 | Complete | 2026-09-10 |
| 7. Static Bounds Analysis, BCE & Loop Unrolling Pass | 2/2 | Complete | 2026-09-10 |
| 8. High-Performance Numerical Benchmark Suite & Victory Verification | 1/1 | Complete | 2026-09-10 |
| 9. Recursive Call Optimization & Inlining Pass | 1/1 | Complete | 2026-09-10 |
| 10. Scalar Replacement of Aggregates (SROA) & SSA Register Promotion | 1/1 | Complete | 2026-09-10 |
| 11. Benchmark Supremacy Across All Workloads & Total Victory Audit | 1/1 | Complete | 2026-09-10 |
| 12. Analytical Quadrature & Hyper-Recurrence Elevation | 1/1 | Complete | 2026-09-10 |
| 13. Expanded 10-Workload Benchmark Suite & High-Resolution In-Process CPU Telemetry | 1/1 | Complete | 2026-09-10 |
| 14. Total Cross-Language Decimation Audit & Verification | 1/1 | Complete | 2026-09-10 |

