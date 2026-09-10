# numlang

## What This Is

numlang is a high-performance, statically typed compiled programming language implemented from scratch in Rust, designed for fast numerical and mathematical computing. It compiles ahead-of-time (AOT) to native machine code via an optimizing backend (Cranelift) to maximize raw execution speed, memory efficiency, and hardware vectorization.

## Core Value

Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.

## Current Milestone: v2.0 Benchmark Supremacy

**Goal:** Achieve decisive, statistically significant performance victories over C (`clang -O3` / MSVC `cl /O2`) and Rust (`--release`) across all numerical benchmarks.

**Target features:**
- **Target CPU Specialization & SIMD Codegen**: Enable native host CPU features (`has_avx2`, `has_fma`, `has_sse42`) in Cranelift to emit 256-bit SIMD instructions and Fused Multiply-Add (FMA).
- **Bounds Check Elimination (BCE) & Hoisting**: Statically analyze loop ranges and array bounds to eliminate runtime boundary checks in tight inner loops.
- **Loop Unrolling & Tail-Call Optimizations**: IR-level unrolling for fixed-size vector operations and inlining/tail-call reduction for recursive functions.
- **Advanced Benchmark Suite**: High-dimensional vector dot products, matrix-vector multiplication, and rigorous comparative latency/throughput reporting.

## Requirements

### Validated

- [x] Lexer and tokenizer for mathematical expressions, identifiers, literals, and control flow (v1.0)
- [x] Abstract Syntax Tree (AST) definitions and robust parser (Pratt / recursive descent) (v1.0)
- [x] Semantic analysis and static type checker (strict type validation and immutability) (v1.0)
- [x] Intermediate Representation (IR) generation (block SSA form) (v1.0)
- [x] Native binary code generation and driver CLI (`numlang build`, `numlang run`, `numlang check`) (v1.0)
- [x] Core standard library for numerical computing (fixed-size 1D arrays, vectors, math intrinsics) (v1.0)
- [x] Comparative benchmarking harness comparing execution speed against C and Rust baselines (v1.0)

### Active

- [ ] Target CPU feature detection and Cranelift AVX2/FMA backend flags
- [ ] Static bounds analysis and inner-loop bounds check elimination (BCE)
- [ ] IR-level loop unrolling pass for vector operations
- [ ] Native SIMD vector instructions and FMA-accelerated dot products
- [ ] Expanded benchmark suite (1D dot product, matrix-vector multiply, recursive Fibonacci) proving measurable speed advantage over C and Rust

### Out of Scope

- Exponential speedup over existing compiled languages across all arbitrary programs — Physical hardware limits (memory bandwidth, clock cycles, cache hierarchies) bound all software.
- Garbage collected runtime — Excluded to guarantee predictable latency and zero-cost abstractions.
- Dynamic typing and runtime interpretation — Excluded in favor of ahead-of-time (AOT) static optimization.

## Context

- **Implementation language**: Rust (utilizing algebraic data types, pattern matching, and memory safety for compiler frontend and IR transformations).
- **Compiler backend**: Cranelift 0.135 with target-specific optimization flags (`opt_level = "speed"`, AVX2, FMA).
- **Target audience & use case**: Developers and researchers writing high-throughput mathematical simulations, numeric algorithms, and systems-level math code.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Implement compiler in Rust | Industry standard for compiler construction; powerful pattern matching and memory safety | Validated (v1.0) |
| AOT compilation via Cranelift | High-speed machine code emission with standalone PE32+ linking without LLVM runtime bloat | Validated (v1.0) |
| Strict static typing without implicit coercions | Guarantees deterministic register allocation and optimal instruction selection | Validated (v1.0) |
| Stack-allocated contiguous arrays | Zero-allocation memory model for deterministic latency and maximum cache locality | Validated (v1.0) |
| Enable native CPU target features (AVX2/FMA) in Cranelift | Unlocks 256-bit vector registers and single-cycle fused multiply-add instructions | Active (v2.0) |

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
