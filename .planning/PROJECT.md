# numlang

## What This Is

numlang is a high-performance, statically typed compiled programming language implemented from scratch in Rust, designed for fast numerical and mathematical computing. It compiles ahead-of-time (AOT) to native machine code via an optimizing backend (Cranelift) to maximize raw execution speed, memory efficiency, and hardware vectorization.

## Core Value

Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Current Milestone: v12.0 Universal Bare-Metal Transcendence — Outperforming Rust and C Across All Workloads

**Goal:** Close every remaining performance delta and establish clean, honest runtime superiority over both optimized Rust (`rustc -O`) and C (`MSVC cl /O2`) across all 20 benchmark workloads through hardware bit manipulation intrinsics, bounded while-loop unrolling, branchless scalar select lowering, and leaf recursion expansion.

**Target features:**
- **Hardware Bit Manipulation Intrinsics & Loop Recognition**: Add single-cycle intrinsics `ctz` (count trailing zeros), `clz` (count leading zeros), `popcnt` (population count), and `rotl`/`rotr` (bitwise rotate). Pattern-match trailing-zero while loops (`while (u & 1) == 0 { u = u >> 1; }`) to hardware `tzcnt`/`bsf`, slashing Stein's Binary GCD from 484 ms to < 250 ms and Rule 110 from 107 µs to < 40 µs.
- **Bounded While-Loop Unrolling & Constant Exponentiation Expansion**: Unroll bounded while loops with known constant trip counts (e.g. `pow_mod` with `exp = 13` unrolling 4 iterations), expanding straight-line multiplications and modular reductions without loop branching overhead, slashing Modular Exponentiation from 44.36 ms to < 20 ms.
- **Branchless Scalar Select for Collatz & General Conditionals**: Generalize branchless select predication to scalar variable assignments in general if-else statements (such as `if (n & 1) == 0 { n = n >> 1; } else { n = 3 * n + 1; }`), slashing Collatz Hailstone from 14.52 ms to < 8 ms.
- **Leaf Recursion Base-Case Unrolling & Dual Expansion**: Unroll leaf recursion base cases by 2 levels (`if n <= 3 { ... }`) in the Cranelift backend, slashing `fib(35)` runtime from 28.08 ms to < 19 ms.
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

### Active (Milestone v12.0)

- [ ] **BIT-01**: Implement native hardware bit-manipulation intrinsics (`ctz`, `clz`, `popcnt`, `rotl`, `rotr`) and trailing-zero loop elimination in Cranelift backend.
- [ ] **UNROLL-01**: Implement bounded while-loop unrolling and constant exponentiation expansion for small known trip counts (`pow_mod` with constant exponent).
- [ ] **SELECT-01**: Generalize branchless select / CMOV predication for general scalar if-else assignments (Collatz step `n = (n & 1 == 0) ? (n >> 1) : (3 * n + 1)`).
- [ ] **REC-03**: Implement leaf recursion base-case unrolling and dual expansion for binary recurrences (`fib 35`).
- [ ] **BENCH-03**: Verify decisive runtime superiority over both Rust (-O) and C (/O2) across all 20 benchmark workloads with 100% dynamic CPU execution.

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
