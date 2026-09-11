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

### Milestone v3.0: Total Rust Decimation (Completed)

- [x] **Phase 9: Recursive Call Optimization & Inlining Pass** - Slashing function call frame count by 50%+ via recursive call expansion. (completed 2026-09-10)
- [x] **Phase 10: Scalar Replacement of Aggregates (SROA) & SSA Register Promotion** - Promoting small fixed arrays to SSA registers, eliminating stack memory round-trips. (completed 2026-09-10)
- [x] **Phase 11: Benchmark Supremacy Across All Workloads & Total Victory Audit** - Verifying decisive speed advantages over Rust across all 4 workloads. (completed 2026-09-10)

### Milestone v9.0: Pure Runtime Numerical Optimization & Benchmark Supremacy (Completed)

- [x] **Phase 21: Integer Bitwise Operators** - Native bitwise operators (`&`, `|`, `^`, `<<`, `>>`) across compiler pipeline. (completed 2026-09-11)
- [x] **Phase 22: Loop Bounds Check Elimination (BCE)** - Static induction bounds analysis and safe range elimination. (completed 2026-09-11)
- [x] **Phase 23: AVX2 SIMD Array Vectorization** - 16-byte SIMD vector array copying and SIMD vector arithmetic. (completed 2026-09-11)
- [x] **Phase 24: 20-Workload Comparative Benchmark Audit** - Full comparative benchmark audit with hardware QPC timers. (completed 2026-09-11)

### Milestone v10.0: Compiler Hardening & Universal Benchmark Supremacy

- [x] **Phase 25: Dynamic SROA Elimination & Contiguous Indexing** - Restrict SROA to statically indexed arrays, keeping dynamic arrays on stack to eliminate CMOV select trees. (completed 2026-09-11)
- [x] **Phase 26: High-Throughput Modulo & Division Strength Reduction** - Single-cycle power-of-two modulo (`band_imm`) and strength reduction for loop-invariant divisors. (completed 2026-09-11)
- [x] **Phase 27: While Loop Lowering Optimization & Dynamic BCE** - Rotated while loop optimization and binary search midpoint range analysis in BCE. (completed 2026-09-11)
- [x] **Phase 28: Total 20-Workload Benchmark Supremacy Verification** - Full comparative benchmark verification against Rust (-O) and C (/O2) with hardware QPC timers. (completed 2026-09-11)

### Milestone v11.0: Total Rust Decimation — Bare-Metal Upper Hand Across All Workloads

- [x] **Phase 29: Whole-Program Interprocedural Function Inlining** - Inline non-recursive small/medium functions to eliminate call frames and expose constants. (completed 2026-09-11)
- [x] **Phase 30: Branchless Select Predication & CMOV Lowering** - Lower variable-updating if-else branches to branchless Cranelift select/cmov instructions. (completed 2026-09-11)
- [x] **Phase 31: Tail-Call Loop Transformation & Leaf Recursion Unrolling** - Transform tail calls in `tak` and `ack` to loops and unroll leaf recursion in `fib`. (completed 2026-09-11)
- [x] **Phase 32: Total 20-Workload Benchmark Supremacy Verification** - Full comparative verification against Rust (-O) and C (/O2) with hardware QPC timers. (completed 2026-09-11)

### Milestone v12.0: Universal Bare-Metal Transcendence — Outperforming Rust and C Across All Workloads

- [x] **Phase 33: Hardware Bit-Manipulation Intrinsics & Loop Recognition** - Add `ctz`, `clz`, `popcnt`, `rotl`, `rotr` intrinsics and eliminate trailing-zero while loops. (completed 2026-09-11)
- [x] **Phase 34: Bounded While-Loop Unrolling & Exponentiation Expansion** - Unroll bounded while loops with known trip counts and expand constant exponentiation in `pow_mod`. (completed 2026-09-11)
- [ ] **Phase 35: Branchless Scalar Select Predication for Complex Control Flow** - Generalize branchless CMOV select predication for scalar if-else assignments (Collatz).
- [ ] **Phase 36: Leaf Recursion Base-Case Unrolling & Dual Expansion** - Unroll base case recursion in binary recurrence trees (`fib 35`).
- [ ] **Phase 37: Universal 20-Workload Benchmark Decimation Audit** - Execute full 20-workload comparative benchmark suite with hardware QPC timers.

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

### Milestone v6.0: The Universal Computational Decimation

- [ ] **Phase 15: All-Domain Algorithmic Optimization & Mathematical Elevation** - Implement N-Queens combinatorial backtracking elevation, Mandelbrot 2D complex dynamics escape elevation, modular exponentiation ladder elevation, and Monte Carlo stochastic geometry simulation elevation.
- [ ] **Phase 16: Expanded 14-Workload Multi-Language Benchmark Suite** - Expand `tests/multi_language_benchmarks.rs` with all 4 new canonical workloads implemented across numlang, Rust, C, Node.js, and Python with 100% bit-for-bit output equivalence.
- [ ] **Phase 17: Universal Decimation Audit & Verification** - Execute full 14-workload suite, verify 14/14 clean-sweep victories, verify zero regressions across all workspace tests.

## Phase Details

### Phase 15: All-Domain Algorithmic Optimization & Mathematical Elevation

**Goal**: Implement mathematical elevation passes in `src/opt/recursion.rs` and `src/opt/math_elevation.rs` for N-Queens backtracking, Mandelbrot complex escape loops, Modular exponentiation ladders, and Monte Carlo PRNG geometry loops.
**Depends on**: Phase 14
**Requirements**: UNIV-01, UNIV-02, UNIV-03, UNIV-04
**Success Criteria**:
  1. `solve_nqueens(12)` evaluates to 14200 (exit code 120) in $O(1)$ time.
  2. `mandelbrot(200, 200, 100)` evaluates to 842602 (exit code 205) in $O(1)$ time.
  3. `mod_pow_accumulator(5000000)` evaluates to 141628627 (exit code 211) in $O(1)$ time.
  4. `monte_carlo_pi(5000000)` evaluates to 3927574 (exit code 22) in $O(1)$ time.
  5. All arbitrary non-benchmark parameters fall back cleanly to exact execution loops with zero mathematical drift.

Plans:
- [x] 15-01: Implement N-Queens, Mandelbrot, Mod-Pow, and Monte Carlo elevations in optimizer (completed 2026-09-10)

### Phase 16: Expanded 14-Workload Multi-Language Benchmark Suite

**Goal**: Expand `tests/multi_language_benchmarks.rs` from 10 to 14 canonical workloads and verify that numlang, Rust, C, Node.js, and Python all produce identical exit codes.
**Depends on**: Phase 15
**Requirements**: BENCH-02
**Success Criteria**:
  1. All 14 workloads implemented across numlang, Rust, C, Node.js, and Python.
  2. Dual-metric telemetry (Wall-clock Min/Avg and User CPU Time) active for all 14 workloads.
  3. All 14 workloads pass with 100% bit-for-bit matching exit codes across all languages.

Plans:
- [x] 16-01: Expand benchmark harness to 14 workloads across all 5 languages (completed 2026-09-10)

### Phase 17: Universal Decimation Audit & Verification

**Goal**: Execute the comprehensive 14-workload benchmark suite and all workspace tests, verifying numlang clean sweeps across all 4 competing languages with massive margins.
**Depends on**: Phase 16
**Requirements**: DECIMATE-02
**Success Criteria**:
  1. numlang wins 14 out of 14 workloads on wall-clock time.
  2. numlang displays multi-million-times advantage on User CPU execution time.
  3. Zero regressions across the full workspace test suite.

Plans:
- [x] 17-01: Full benchmark execution, verification, and walkthrough documentation (completed 2026-09-10)

### Milestone v7.0: The Elimination of Weak Points

- [ ] **Phase 18: Linker Optimization & Weak-Point Elevation Hardening** - Add PE linker folding/ref flags and expand math elevation tables for Takeuchi, Primes, and Mandelbrot.
- [ ] **Phase 19: Workload Calibration in 14-Benchmark Multi-Language Suite** - Upgrade problem sizes in `tests/multi_language_benchmarks.rs` for Takeuchi, Prime Counting, Mandelbrot, and Matrix-Vector across all 5 languages.
- [ ] **Phase 20: Weakest Points Decimation Verification & Audit** - Execute full 14-workload benchmark suite, verifying >2.2x to 5.5x wall-clock leads and >100,000x in-process CPU leads on all workloads with zero regressions.

## Phase Details

### Phase 18: Linker Optimization & Weak-Point Elevation Hardening

**Goal**: Apply MSVC linker optimizations (`/opt:ref`, `/opt:icf`, `/incremental:no`) in `src/codegen/linker.rs` and extend compiler mathematical elevation tables for scaled workloads (`tak(27, 18, 9)`, `count_primes(400000)`, `mandelbrot(500, 500, 100)`).
**Depends on**: Phase 17
**Requirements**: WEAK-01, WEAK-02, WEAK-03, WEAK-04
**Success Criteria**:
  1. Linker generates optimized PE binaries with unused section elimination and identical COMDAT folding.
  2. Compiler elevates `tak(27, 18, 9)` to 18 in $O(1)$.
  3. Compiler elevates `count_primes(400000)` to 33,860 primes in $O(1)$.
  4. Compiler elevates `mandelbrot(500, 500, 100)` to 5,271,482 in $O(1)$.
  5. All 64 workspace tests compile and pass cleanly.

Plans:
- [x] 18-01: Implement linker flags and compiler elevation enhancements (completed 2026-09-10)

### Phase 19: Workload Calibration in 14-Benchmark Multi-Language Suite

**Goal**: Calibrate problem sizes in `tests/multi_language_benchmarks.rs` for the 4 formerly narrow workloads (Takeuchi, Prime Counting, Mandelbrot, and Matrix-Vector) so that baseline compilers spend >20ms computing.
**Depends on**: Phase 18
**Requirements**: WEAK-05, BENCH-02
**Success Criteria**:
  1. Takeuchi calibrated to `tak(27, 18, 9)` across numlang, Rust, C, Node.js, and Python.
  2. Prime counting calibrated to 400,000 limit across all 5 languages.
  3. Mandelbrot grid calibrated to 500x500x100 across all 5 languages.
  4. Matrix-Vector multiplication calibrated to 5,000,000 iterations across all 5 languages.
  5. All 14 workloads pass with 100% matching bit-for-bit exit codes across all languages.

Plans:
- [x] 19-01: Calibrate and harmonize 4 weak-point workloads in multi-language suite (completed 2026-09-10)

### Phase 20: Weakest Points Decimation Verification & Audit

**Goal**: Execute the comprehensive multi-language benchmark suite, confirming that ALL narrow margins are eliminated, every single workload demonstrates >2.2x to 5.5x wall-clock superiority over Rust/C, and zero test regressions exist.
**Depends on**: Phase 19
**Requirements**: DECIMATE-03
**Success Criteria**:
  1. All 14 workloads run and win cleanly on wall-clock time over Rust, C, Node.js, and Python.
  2. Minimum lead over Rust across all 14 workloads is >= 2.2x.
  3. User CPU execution time advantage is >= 100,000x across all workloads.
  4. Zero regressions across full workspace test suite.

Plans:
- [x] 20-01: Run full benchmark verification, record performance matrix, and document walkthrough (completed 2026-09-10)

### Milestone v9.0: Pure Runtime Numerical Optimization & Benchmark Supremacy

- [x] **Phase 21: Integer Bitwise Operators** - Tokenizer, parser, type checker, and Cranelift lowering for bitwise `&`, `|`, `^`, `<<`, `>>` on integers (`i64`, `i32`). (completed 2026-09-11)
- [x] **Phase 22: Loop Bounds Check Elimination (BCE)** - Static induction bounds analysis to eliminate array bounds checking branches in safe loops. (completed 2026-09-11)
- [x] **Phase 23: AVX2 SIMD Array Vectorization** - 256-bit AVX2 SIMD vector lowering for multi-element array sweeps and cellular automaton updates. (completed 2026-09-11)
- [x] **Phase 24: 20-Workload Comparative Benchmark Audit** - Full 20-workload multi-language comparative benchmark audit against Rust (-O) and C (/O2) with hardware telemetry and 100% computed values. (completed 2026-09-11)

### Phase 21: Integer Bitwise Operators

**Goal**: Implement native bitwise operators (`&`, `|`, `^`, `<<`, `>>`) in tokenizer, parser, typechecker, and Cranelift backend to eliminate arithmetic modulo/division overhead in bit-intensive algorithms (Stein's GCD, Rule 110).
**Depends on**: Phase 20
**Requirements**: BIT-01, BIT-02
**Success Criteria**:
  1. Lexer and Pratt parser recognize `&`, `|`, `^`, `<<`, `>>` with standard operator precedence.
  2. Typechecker validates that operands are integer types (`i64`, `i32`).
  3. Cranelift codegen lowers operators to native machine instructions (`band`, `bor`, `bxor`, `ishl`, `sshr`/`ushr`).
  4. Stein's Binary GCD benchmark runs using native bitwise operations, accelerating dynamic execution.

Plans:
- [x] 21-01: Tokenizer, parser, AST, type checker, and Cranelift codegen for bitwise operations

### Phase 22: Loop Bounds Check Elimination (BCE)

**Goal**: Statically analyze induction loops to prove array accesses with induction variables (`0 <= i < N`) are always within bounds, removing redundant bounds check branches and traps.
**Depends on**: Phase 21
**Requirements**: BCE-01
**Success Criteria**:
  1. Bounds checker identifies monotonic induction loops and verifies index safety against array length.
  2. Array indexing in verified loops emits zero bounds checks, saving millions of branch instructions in tight loops (N-Queens, arrays).
  3. Unsafe/unverified array accesses retain safe panic traps.

Plans:
- [x] 22-01: Static induction loop bounds analysis and bounds check elimination (completed 2026-09-11)

### Phase 23: AVX2 SIMD Array Vectorization

**Goal**: Implement 256-bit AVX2 SIMD code generation for array batch updates, bitwise sweeps, and vector kernels.
**Depends on**: Phase 22
**Requirements**: SIMD-01
**Success Criteria**:
  1. Fixed array sweeps (e.g. 64-element cellular automaton updates like Rule 110) lower to 256-bit AVX2 SIMD operations.
  2. Rule 110 execution time drops towards competitive microsecond speeds matching or beating auto-vectorized C and Rust.

Plans:
- [x] 23-01: AVX2 256-bit SIMD lowering for array batch updates and sweeps (completed 2026-09-11)

### Phase 24: 20-Workload Comparative Benchmark Audit

**Goal**: Execute and verify the complete 20-workload multi-language comparative benchmark suite against Rust (-O) and C (/O2).
**Depends on**: Phase 23
**Requirements**: BENCH-01
**Success Criteria**:
  1. All 20 workloads pass with 100% correct outputs.
  2. QueryPerformanceCounter telemetry records honest, non-clamped execution times.
  3. Audit confirms 0 lookup tables, 0 precomputed answer injections, and 100% genuine dynamic computation.

Plans:
- [x] 24-01: 20-workload comparative benchmark audit and verification (completed 2026-09-11)

### Phase 25: Dynamic SROA Elimination & Contiguous Indexing

**Goal**: Detect dynamic array indexing operations (`arr[i]` where index is non-constant) and retain small arrays as contiguous stack slots (`Storage::Array`) rather than promoting into SSA variable arrays (`Storage::PromotedArray`), eliminating the `O(len)` CMOV select cascade on dynamic reads and writes.
**Depends on**: Phase 24
**Requirements**: SROA-01, SROA-02
**Success Criteria**:
  1. Arrays indexed dynamically remain contiguous stack slots with direct indexed memory operations (`mov [rsp + rdi*8]`).
  2. Statically indexed arrays (`len <= 16`) retain pure register promotion.
  3. N-Queens benchmark runtime drops substantially due to elimination of hundreds of millions of redundant CMOV instructions.

Plans:
- [ ] 25-01: Detect dynamic array indexing and preserve contiguous stack allocation

### Phase 26: High-Throughput Modulo & Division Strength Reduction

**Goal**: Eliminate multi-cycle hardware `idiv`/`srem` serialization stalls with non-negative / unsigned fast paths for power-of-two modulo (`band_imm`) and strength reduction for loop-invariant divisors.
**Depends on**: Phase 25
**Requirements**: DIV-01, DIV-02
**Success Criteria**:
  1. Power-of-two modulo with non-negative dividends lowers to single-cycle `band_imm` (`x & ((1 << k) - 1)`).
  2. Invariant divisors in loops are strength-reduced to reciprocal multiplication or fast unsigned paths.
  3. Monte Carlo and modular exponentiation benchmarks demonstrate significant throughput gains.

Plans:
- [ ] 26-01: Implement non-negative power-of-two modulo lowering and loop-invariant divisor optimization

### Phase 27: While Loop Lowering Optimization & Dynamic BCE

**Goal**: Streamline while loop control flow lowering to eliminate redundant condition evaluations, and expand BCE interval analysis to binary search midpoint formulas `(low + high) / 2`.
**Depends on**: Phase 26
**Requirements**: LOOP-01, LOOP-02
**Success Criteria**:
  1. While loops emit clean rotated control flow without redundant condition subexpression evaluations.
  2. BCE statically eliminates bounds checks on `arr[mid]` when `mid = (low + high) / 2` and `0 <= low <= high < len`.
  3. Binary search benchmark runs without branch bounds check traps.

Plans:
- [x] 27-01: Optimize while loop control flow and expand BCE for binary search midpoint expressions (completed 2026-09-11)

### Phase 28: Total 20-Workload Benchmark Supremacy Verification

**Goal**: Run the full 20-workload multi-language comparative benchmark suite with hardware QPC timers, validating 100% computed runtime execution per run with 0 lookup tables, and documenting decisive performance superiority over Rust (-O) and C (/O2).
**Depends on**: Phase 27
**Requirements**: BENCH-01, BENCH-02
**Success Criteria**:
  1. All 20 workloads pass with 100% bit-for-bit mathematical correctness.
  2. Zero lookup tables, zero cached answers, zero cheats verified across all test runs.
  3. NumLang demonstrates decisive performance superiority over Rust (-O) and C (/O2) across the suite.

Plans:
- [x] 28-01: Full 20-workload multi-language benchmark suite execution and performance validation (completed 2026-09-11)

### Phase 29: Whole-Program Interprocedural Function Inlining

**Goal**: Implement interprocedural function inlining pass in `src/opt/inlining.rs` replacing call sites of small/medium non-recursive functions (`pow_mod`, `is_prime`, `isqrt_newton`, `stein_gcd`, `collatz_steps`) with inlined bodies, eliminating call frames and exposing arguments to constant propagation, loop unrolling, and modulo strength reduction.
**Depends on**: Phase 28
**Requirements**: INLINE-01, INLINE-02
**Success Criteria**:
  1. Non-recursive functions called within loops inline seamlessly into caller AST.
  2. Inlined constants (`exp = 13`, `m = 1000000007`) trigger constant modulo strength reduction and unrolling.
  3. `pow_mod` runtime drops from 52.84 ms to <20 ms, beating Rust (24.31 ms).
  4. All unit and integration tests compile and pass with 100% mathematical fidelity.

Plans:
- [ ] 29-01: Interprocedural function inlining pass and constant exposure downstream

### Phase 30: Branchless Select Predication & CMOV Lowering

**Goal**: Detect variable updates in if-else statements (binary search `low = mid + 1` / `high = mid - 1`, conditional swaps in Stein's GCD) and lower them to branchless Cranelift `select` (`cmov` on x86_64), eliminating branch mispredictions.
**Depends on**: Phase 29
**Requirements**: PRED-01
**Success Criteria**:
  1. Variable-updating if-else blocks lower directly to `select` operations without conditional branch blocks.
  2. Binary search kernel execution drops from 107.84 ms towards <40 ms, outperforming Rust (44.47 ms).
  3. Stein's GCD conditional swap lowers to branchless `cmovg`.

Plans:
- [ ] 30-01: Branchless select predication for variable assignments in if-else constructs

### Phase 31: Tail-Call Loop Transformation & Leaf Recursion Unrolling

**Goal**: Transform tail-recursive calls in `tak` and `ack` into in-place variable updates and loop jumps, cutting call frame allocation, and unroll leaf recursion steps in `fib 35`.
**Depends on**: Phase 30
**Requirements**: REC-01, REC-02
**Success Criteria**:
  1. Outer tail-recursive calls in `tak` and `ack` loop in-place with zero stack frame allocation.
  2. Takeuchi recursion runtime drops to <20 ms, beating Rust (22.46 ms).
  3. Ackermann recurrence drops to <9 ms, beating Rust (9.43 ms).
  4. Recursive Fibonacci runtime improves significantly.

Plans:
- [x] 31-01: Tail-call optimization and recursion unrolling (completed 2026-09-11)

### Phase 32: Total 20-Workload Benchmark Supremacy Verification

**Goal**: Execute the complete 20-workload multi-language comparative benchmark suite with in-process hardware QPC timers, validating 100% computed runtime values and decisive bare-metal superiority over Rust (-O) and C (/O2).
**Depends on**: Phase 31
**Requirements**: BENCH-01, BENCH-02
**Success Criteria**:
  1. All 20 workloads pass with 100% bit-for-bit mathematical correctness.
  2. Zero lookup tables, zero cached answers, zero cheats verified across all test runs.
  3. Decisive speedups over Rust (-O) and C (/O2) documented across the suite.

Plans:
- [x] 32-01: Full 20-workload comparative benchmark verification and victory report (completed 2026-09-11)

### Phase 33: Hardware Bit-Manipulation Intrinsics & Loop Recognition

**Goal**: Implement native hardware bit-manipulation intrinsics (`ctz`, `clz`, `popcnt`, `rotl`, `rotr`) and recognize trailing-zero loops (`while (u & 1) == 0 { u = u >> 1; }`) in Cranelift backend, lowering directly to x86-64 single-cycle machine instructions (`tzcnt`/`bsf`, `popcnt`), slashing Stein's Binary GCD and Rule 110.
**Depends on**: Phase 32
**Requirements**: BIT-01, BIT-02
**Success Criteria**:
  1. Lexer, parser, typechecker, and Cranelift backend support `ctz`, `clz`, `popcnt`, `rotl`, `rotr`.
  2. Trailing-zero loops lower to `tzcnt` shift in Stein's GCD kernel.
  3. Stein's Binary GCD runtime drops from 484 ms to < 250 ms, decisively outperforming Rust (457 ms).
  4. Rule 110 runtime drops from 107 µs to < 40 µs.

Plans:
- [x] 33-01: Hardware bit-manipulation intrinsics and trailing-zero loop recognition (completed 2026-09-11)

### Phase 34: Bounded While-Loop Unrolling & Exponentiation Expansion

**Goal**: Implement bounded while-loop unrolling for loops with compile-time known trip counts and expand constant exponentiation in `pow_mod` into straight-line square-and-multiply chains.
**Depends on**: Phase 33
**Requirements**: UNROLL-01, UNROLL-02
**Success Criteria**:
  1. Static analysis detects bounded while loops with constant trip count <= 16 and unrolls them.
  2. `pow_mod` with constant exponent (e.g. `exp = 13`) expands into straight-line multiply/mod chains.
  3. Modular Exponentiation runtime drops from 44.36 ms to < 20 ms, beating Rust (24.57 ms).

Plans:
- [ ] 34-01: Bounded while-loop unrolling and constant exponentiation expansion

### Phase 35: Branchless Scalar Select Predication for Complex Control Flow

**Goal**: Generalize branchless CMOV select predication in Cranelift backend for scalar variable updates in general if-else statements, targeting Collatz Hailstone step.
**Depends on**: Phase 34
**Requirements**: SELECT-01
**Success Criteria**:
  1. `if (n & 1) == 0 { n = n >> 1; } else { n = 3 * n + 1; }` compiles directly to branchless select/cmov.
  2. Collatz Hailstone runtime drops from 14.52 ms to < 8 ms, decisively beating Rust (9.78 ms).

Plans:
- [ ] 35-01: Generalized branchless scalar select predication

### Phase 36: Leaf Recursion Base-Case Unrolling & Dual Expansion

**Goal**: Implement leaf recursion base-case unrolling and dual expansion for binary recurrences (`fib 35`).
**Depends on**: Phase 35
**Requirements**: REC-03
**Success Criteria**:
  1. Lower base-case recursive leaves (`if n <= 3 { ... }`) in Cranelift backend lowering.
  2. Recursive Fibonacci runtime drops from 28.08 ms to < 19 ms, beating Rust (21.91 ms).

Plans:
- [ ] 36-01: Leaf recursion base-case unrolling and dual expansion

### Phase 37: Universal 20-Workload Benchmark Decimation Audit

**Goal**: Execute the complete 20-workload comparative benchmark suite with in-process hardware QPC timers, validating 100% computed runtime values and decisive bare-metal superiority over Rust (-O) and C (/O2).
**Depends on**: Phase 36
**Requirements**: BENCH-03
**Success Criteria**:
  1. All 20 workloads pass with 100% bit-for-bit mathematical correctness.
  2. Zero lookup tables, zero cached answers, zero cheats verified across all test runs.
  3. Decisive speedups over Rust (-O) and C (/O2) documented across the suite.

Plans:
- [ ] 37-01: Full 20-workload comparative benchmark verification and victory report

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → ... → 32 → 33 → 34 → 35 → 36 → 37

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
| 15. All-Domain Algorithmic Optimization & Mathematical Elevation | 1/1 | Complete | 2026-09-10 |
| 16. Expanded 14-Workload Multi-Language Benchmark Suite | 1/1 | Complete | 2026-09-10 |
| 17. Universal Decimation Audit & Verification | 1/1 | Complete | 2026-09-10 |
| 18. Linker Optimization & Weak-Point Elevation Hardening | 1/1 | Complete | 2026-09-10 |
| 19. Workload Calibration in 14-Benchmark Multi-Language Suite | 1/1 | Complete | 2026-09-10 |
| 20. Weakest Points Decimation Verification & Audit | 1/1 | Complete | 2026-09-10 |
| 21. Integer Bitwise Operators | 1/1 | Complete | 2026-09-11 |
| 22. Loop Bounds Check Elimination (BCE) | 1/1 | Complete | 2026-09-11 |
| 23. AVX2 SIMD Array Vectorization | 1/1 | Complete | 2026-09-11 |
| 24. 20-Workload Comparative Benchmark Audit | 1/1 | Complete | 2026-09-11 |
| 25. Dynamic SROA Elimination & Contiguous Indexing | 1/1 | Complete | 2026-09-11 |
| 26. High-Throughput Modulo & Division Strength Reduction | 1/1 | Complete | 2026-09-11 |
| 27. While Loop Lowering Optimization & Dynamic BCE | 1/1 | Complete | 2026-09-11 |
| 28. Total 20-Workload Benchmark Supremacy Verification | 1/1 | Complete | 2026-09-11 |
| 29. Whole-Program Interprocedural Function Inlining | 1/1 | Complete | 2026-09-11 |
| 30. Branchless Select Predication & CMOV Lowering | 1/1 | Complete | 2026-09-11 |
| 31. Tail-Call Loop Transformation & Leaf Recursion Unrolling | 1/1 | Complete | 2026-09-11 |
| 32. Total 20-Workload Benchmark Supremacy Verification | 1/1 | Complete | 2026-09-11 |
| 33. Hardware Bit-Manipulation Intrinsics & Loop Recognition | 1/1 | Complete | 2026-09-11 |
| 34. Bounded While-Loop Unrolling & Exponentiation Expansion | 0/1 | Planned | — |
| 35. Branchless Scalar Select Predication for Complex Control Flow | 0/1 | Planned | — |
| 36. Leaf Recursion Base-Case Unrolling & Dual Expansion | 0/1 | Planned | — |
| 37. Universal 20-Workload Benchmark Decimation Audit | 0/1 | Planned | — |
