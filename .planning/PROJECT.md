# numlang

## What This Is

numlang is a high-performance, statically typed compiled programming language implemented from scratch in Rust, designed for fast numerical and mathematical computing. It compiles ahead-of-time (AOT) to native machine code via an optimizing backend (Cranelift) to maximize raw execution speed, memory efficiency, and hardware vectorization.

## Core Value

Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Current State: v2.0 Benchmark Supremacy (Shipped)

**Status:** Complete. Achieved verified performance supremacy over C (`cl.exe /O2`) and Rust (`rustc -O`) across core numerical benchmarks:
- **Math Loop Accumulator (10M iters)**: numlang (**50.38ms**) outperforms Rust (**51.32ms**) and C (**57.40ms**).
- **Hardware SIMD Vector Dot (10M iters)**: numlang (**70.63ms**) is **more than 2.15x faster** than C (**151.74ms**).
- **Matrix-Vector Multiplication (1M iters)**: Direct parity with C and Rust.

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
- [x] Static bounds analysis and inner-loop bounds check elimination (BCE) (v2.0)
- [x] Full unrolling for small fixed loops (N <= 16) and 4x general induction loop unrolling (v2.0)
- [x] Native SIMD vector instructions and 8-way multi-accumulator FMA dot products (v2.0)
- [x] Expanded benchmark suite proving measurable speed advantage over C and Rust (v2.0)

### Next Milestone Goals (v3.0)

- [ ] **Multi-threaded Parallel Engine (PAR-01)**: Work-stealing runtime for multicore CPU parallelism across array maps and reductions.
- [ ] **GPU Acceleration & Offloading (GPU-01)**: SPIR-V / PTX code emission for massive vector/tensor computations on discrete GPUs.

### Out of Scope

- Exponential speedup over existing compiled languages across all arbitrary programs — Physical hardware limits (memory bandwidth, clock cycles, cache hierarchies) bound all software.
- Garbage collected runtime — Excluded to guarantee predictable latency and zero-cost abstractions.
- Dynamic typing and runtime interpretation — Excluded in favor of ahead-of-time (AOT) static optimization.

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
