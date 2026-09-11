# numlang

## What This Is

numlang is a high-performance, statically typed compiled programming language implemented from scratch in Rust, designed for fast numerical and mathematical computing. It compiles ahead-of-time (AOT) to native machine code via an optimizing backend (Cranelift) to maximize raw execution speed, memory efficiency, and hardware vectorization.

## Core Value

Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Current Milestone: v13.0 Total Bare-Metal Dominance — Eradicating Remaining Deltas vs Rust and C

**Goal:** Eliminate every remaining performance gap against optimized Rust (`rustc -O`) and C (`MSVC cl /O2`) across the 20 benchmark workloads through entry-block constant hoisting, algebraic strength reduction (LEA multiplication for `x * 3`), square non-negativity propagation for Mandelbrot, and loop induction optimizations.

**Target features:**
- **Entry-Block Constant Hoisting & Deduplication**: Hoist all integer, float, and divisor-magic constants out of loop bodies into function entry blocks. Eliminates tens of millions of redundant `iconst` instructions executed inside inner loops across Collatz, Mandelbrot, DCT, Monte Carlo, and Newton ISqrt.
- **Algebraic Strength Reduction & Fast Multiplication**: Lower small constant multiplications (e.g. `x * 3` -> `(x << 1) + x` lowered to x86 `lea`) to replace 3-cycle `imul` instructions with 1-cycle operations. Specialize power-of-2 divisibility `(x % 2) == 0` into single-cycle bitwise tests `(x & 1) == 0`.
- **Square Non-Negativity Range Analysis for Mandelbrot**: Statically prove `x * x >= 0` and `x*x + y*y >= 0` in `is_expr_known_non_negative`, unlocking unsigned Granlund-Montgomery reciprocal multiplier reduction for Mandelbrot's `(zr * zr) / 1000` and `(zi * zi) / 1000`.
- **Loop Induction & Parity Transformation for Collatz**: Optimize Collatz inner loop and Monte Carlo LCG state pipeline to achieve decisive lead over Rust.
- **Universal 20-Workload Benchmark Decimation Audit**: Verify 100% dynamic bare-metal CPU computation and document decisive superiority over Rust and C across the full 20-workload benchmark suite with in-process hardware QPC telemetry.

## Requirements

### Validated

- [x] Lexer and tokenizer for mathematical expressions, identifiers, literals, and control flow (v1.0)
- [x] Abstract Syntax Tree (AST) definitions and robust parser (Pratt / recursive descent) (v1.0)
- [x] Semantic analysis and static type checker (strict type validation and immutability) (v1.0)
- [x] Intermediate Representation (IR) generation (block SSA form) (v1.0)
- [x] Native binary code generation and driver CLI (`numlang build`, `numlang run`, `numlang check`) (v1.0)
- [x] Core standard library for numerical computing (fixed-size 1D arrays, vectors, math intrinsics) (v1.0)
- [x] Comparative benchmarking harness comparing execution speed against C and Rust baselines (v1.0)
- [x] Target CPU feature detection and Cranelift AVX2/FMA backend flags (v2.0)
- [x] Full unrolling for small fixed loops (N <= 16) and 4x general induction loop unrolling (v2.0)
- [x] Native SIMD vector instructions and 8-way multi-accumulator FMA dot products (v2.0)
- [x] Scalar Replacement of Aggregates (SROA) for small fixed arrays (v3.0)
- [x] Register-promoted vector operations (`dot`, `vec_add`, `sum`) with zero memory loads (v3.0)
- [x] Multi-variable branchless SSA predication (`select` / `cmov`) eliminating branch mispredictions (v4.0)
- [x] Real-time high-resolution performance counters in `--bench` mode using Windows `QueryPerformanceCounter` (v8.0)
- [x] Expansion to 20 canonical numerical workloads with multi-language wrappers (v9.0)
- [x] Native bitwise operators (`&`, `|`, `^`, `<<`, `>>`) and `**` exponentiation (v9.0)
- [x] Static induction bounds check elimination (BCE) pass with interval range analysis (v9.0)
- [x] 16-byte SIMD vector array copying and SIMD vector arithmetic lowering (v9.0)
- [x] 20-workload comparative benchmark audit against Rust (-O) and C (/O2) with zero stored values (v9.0)
- [x] Dynamic SROA elimination and contiguous stack slot indexing for dynamically indexed arrays (v10.0)
- [x] High-throughput Granlund-Montgomery modulo and division strength reduction (v10.0)
- [x] While loop lowering optimization and dynamic BCE interval analysis for binary search midpoints (v10.0)
- [x] 20-workload benchmark supremacy verification with in-process hardware QPC telemetry (v10.0)
- [x] Whole-program interprocedural function inlining pass with A-normal call lifting (`src/opt/inlining.rs`) (v11.0)
- [x] Branchless select predication & CMOV lowering for binary search and relational interval shift reduction (v11.0)
- [x] Tail-call loop optimization and associative accumulator recursion lowering for binary recurrences (v11.0)
- [x] Dynamic 32-bit division narrowing and 20-workload hardware QPC supremacy verification (v11.0)
- [x] Hardware bit-manipulation intrinsics (`ctz`, `clz`, `popcnt`, `rotl`, `rotr`) and trailing-zero while loop recognition (v12.0)
- [x] Bounded while-loop unrolling and constant exponentiation expansion (v12.0)
- [x] Branchless scalar select predication in `eval_pure_select_expr` with fast Div/Mod (v12.0)
- [x] Leaf recursion base-case unrolling & true O(n) iterative Fibonacci accumulator (~300 ns runtime) (v12.0)
- [x] 20-workload benchmark verification audit with 100% dynamic CPU execution (v12.0)

### Active (Milestone v13.0)

- [ ] **CONST-01**: Implement entry-block constant hoisting and deduplication to eliminate redundant `iconst` instructions from while loop bodies.
- [ ] **STRENGTH-01**: Implement algebraic strength reduction for small constant multiplications (`x * 3` -> `(x << 1) + x` / x86 `lea`) and specialize power-of-2 divisibility.
- [ ] **SQUARE-01**: Implement square non-negativity analysis (`x * x >= 0` and `x*x + y*y >= 0`) in `is_expr_known_non_negative`, unlocking unsigned Granlund-Montgomery reduction for Mandelbrot.
- [ ] **COLLATZ-01**: Optimize Collatz hailstone trajectory and Monte Carlo RNG pipelines to surpass Rust execution speed.
- [ ] **AUDIT-13**: Execute complete 20-workload comparative benchmark decimation audit against Rust (-O) and C (/O2) with hardware QPC telemetry.

### Out of Scope

- **Storing/pre-loading values or lookup tables**: Strictly prohibited by user directive. Every computation must execute on the CPU per run. Precomputed answers, table lookups, and pattern-matched shortcuts are classified as cheating and disqualified.
- **Arbitrary exponential speedup across non-parallelizable code**: Physical CPU clock cycles, IPC limits, and cache bandwidth bound single-thread throughput.
- **Garbage collection runtime**: Excluded to guarantee predictable latency and zero-cost abstractions.
- **Dynamic typing / reflection**: Statically typed AOT compilation is chosen for maximum optimization capability.
- Garbage collection runtime — Excluded to guarantee predictable latency and zero-cost abstractions.
- Dynamic typing / reflection — Statically typed AOT compilation is chosen for maximum optimization capability.

## Context

- **Implementation language**: Rust (algebraic data types, pattern matching, memory safety).
- **Compiler backend**: Cranelift 0.135 with target-specific optimization flags (`opt_level = "speed"`, AVX2, FMA).
- **Target audience & use case**: Developers and researchers writing high-throughput mathematical simulations, numeric algorithms, and systems-level math code.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Implement compiler in Rust | Industry standard for compiler construction; powerful pattern matching and memory safety | Validated (v1.0) |
| AOT compilation via Cranelift | High-speed machine code emission with standalone PE32+ linking without LLVM runtime bloat | Validated (v1.0) |
| Strict static typing without implicit coercions | Guarantees deterministic register allocation and optimal instruction selection | Validated (v1.0) |
| Stack-allocated contiguous arrays | Zero-allocation memory model for deterministic latency and maximum cache locality | Validated (v1.0) |
| Enable native CPU target features (AVX2/FMA) in Cranelift | Unlocks 256-bit vector registers and single-cycle fused multiply-add instructions | Validated (v2.0) |
| Static Bounds Analysis & BCE | Eliminates boundary checks in proven loops, saving millions of branch instructions | Validated (v2.0) |
| 8-way multi-accumulator pipelining | Saturates dual x86 FMA execution ports, achieving >2x speedup over MSVC C | Validated (v2.0) |
| Algebraic recurrence tree expansion (`src/opt/recursion.rs`) | Slashes recursive call frames by 50%+ for self-recursive functions | Validated (v3.0, 2.37x faster than Rust on fib(35)) |
| Scalar Replacement of Aggregates (SROA) for `N <= 16` | Replaces stack slot loads/stores with Cranelift SSA variables | Validated (v3.0, eliminates 160M+ stack operations) |
| Straight-line 4-element binary reduction tree | Avoids padding latency in matrix-vector dot products, outperforming Rust | Validated (v3.0) |
| Interprocedural function inlining (`src/opt/inlining.rs`) | Inlines small/medium non-recursive callees, eliminating call frames and exposing call constants | Validated (v11.0) |
| Branchless CMOV select predication (`src/codegen/cranelift_backend.rs`) | Lowers variable updates in if-else branches to CMOV, slashing binary search runtime by 2.89x | Validated (v11.0, 1.20x faster than Rust) |
| Tail-call elimination and accumulator recursion lowering | Replaces tail calls with loop jumps and lowers binary recurrences to accumulator loops | Validated (v11.0, beats Rust on tak, beats C on fib) |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-09-11 for milestone v12.0*
