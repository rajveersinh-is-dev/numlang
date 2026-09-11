# numlang

## What This Is

numlang is a high-performance, statically typed compiled programming language implemented from scratch in Rust, designed for fast numerical and mathematical computing. It compiles ahead-of-time (AOT) to native machine code via an optimizing backend (Cranelift) to maximize raw execution speed, memory efficiency, and hardware vectorization.

## Core Value

Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Current Milestone: v11.0 Total Rust Decimation — Bare-Metal Upper Hand Across All Workloads

**Goal:** Implement interprocedural function inlining, branchless CMOV predication, tail-call loop transformations, and bitwise rotate intrinsics to decisively outperform Rust (`rustc -O`) and C (`MSVC cl /O2`) across all 20 canonical numerical benchmarks, with 100% dynamic bare-metal CPU computation per run and zero stored values.

**Target features:**
- **Whole-Program Interprocedural Function Inlining**: Inline small/medium functions (`pow_mod`, `is_prime`, `isqrt_newton`, `stein_gcd`) into call sites, eliminating millions of call frames and exposing call arguments (`exp = 13`, `m = 1000000007`) to constant propagation, loop unrolling, and modulo strength reduction.
- **Branchless Select Predication & CMOV Lowering**: Lower variable-updating if-else constructs (e.g. binary search `low/high` updates, conditional swaps) to Cranelift `select` / `cmov`, eliminating branch mispredictions in search kernels.
- **Tail-Call Loop Transformation & Leaf Recursion Unrolling**: Transform tail-recursive calls in `tak` and `ack` into in-place variable updates and loop jumps, cutting call stack traffic by millions of frames.
- **Total 20-Workload Benchmark Supremacy Verification**: Comprehensive multi-language benchmark suite execution validating speedups across all 20 workloads with hardware QPC timers and bit-for-bit output equivalence.

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

### Active (Milestone v11.0)

- [ ] **INLINE-01**: Interprocedural function inlining pass replacing call sites of small/medium non-recursive functions with inlined bodies, exposing constant arguments and eliminating millions of call frames.
- [ ] **PRED-01**: Branchless select predication for variable assignments in if-else statements (e.g. binary search interval updates, conditional swaps), lowering to `cmov` instructions.
- [ ] **REC-01**: Tail-call loop optimization and leaf recursion unrolling for self-recursive functions (`tak`, `ack`, `fib`), eliminating recursive frame allocation.
- [ ] **BENCH-01**: Universal 20-workload benchmark verification against Rust (-O) and C (/O2) demonstrating superior throughput across all kernels with 100% computed values.

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
*Last updated: 2026-09-11 for milestone v10.0*
