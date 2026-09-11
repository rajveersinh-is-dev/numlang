# numlang

## What This Is

numlang is a high-performance, statically typed compiled programming language implemented from scratch in Rust, designed for fast numerical and mathematical computing. It compiles ahead-of-time (AOT) to native machine code via an optimizing backend (Cranelift) to maximize raw execution speed, memory efficiency, and hardware vectorization.

## Core Value

Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Current Milestone: v9.0 Pure Runtime Numerical Optimization & Benchmark Supremacy

**Goal:** Achieve maximum legitimate bare-metal execution speed over Rust (-O) and C (/O2) across standard numerical benchmarks, with 100% runtime computation (zero lookup tables, zero stored constants) and complete benchmark honesty.

**Target features:**
- **Integer Bitwise Operators**: Native bitwise operators (`&`, `|`, `^`, `<<`, `>>`) in tokenizer, parser, typechecker, and Cranelift backend to eliminate high-overhead modulo/division workarounds in bit-heavy kernels (e.g. Stein's GCD, Rule 110, bit manipulations).
- **Induction Bounds Check Elimination (BCE)**: Statically prove that array accesses inside induction loops (`0 <= i < N`) are within bounds, eliminating redundant branch checks and panic blocks from tight loops like N-Queens and Rule 110.
- **Hardware SIMD Vectorization Engine**: Extend array unrolling and SSA promotion with AVX2 vector instructions for multi-element array operations.
- **Honest Comparative Multi-Language Benchmark Suite**: Continuously track numlang, Rust (-O), and C (/O2) across all 20 workloads with real hardware timers, verifying identical mathematical outputs and zero stored values.

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

### Active (Milestone v9.0)

- [ ] **BIT-01**: Lexer and parser support for bitwise operators (`&`, `|`, `^`, `<<`, `>>`).
- [ ] **BIT-02**: Typechecking and Cranelift lowering for bitwise binary expressions on integer types (`i64`, `i32`).
- [ ] **BCE-01**: Loop induction bounds analysis proving that array indices bounded by the loop condition are safe, omitting runtime `emit_bounds_check` traps.
- [ ] **SIMD-01**: AVX2 256-bit SIMD lowering for parallel array loops (e.g. cellular automaton updates, vector arithmetic).
- [ ] **BENCH-01**: 20-workload comparative benchmark verification against Rust (-O) and C (/O2) with hardware timers, 100% computed per run with zero pre-loaded tables.

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
*Last updated: 2026-09-10 for milestone v2.0*
