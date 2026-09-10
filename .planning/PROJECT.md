# numlang

## What This Is

numlang is a high-performance, statically typed compiled programming language implemented from scratch in Rust, designed for fast numerical and mathematical computing. It compiles ahead-of-time (AOT) to native machine code via an optimizing compiler backend (LLVM/Cranelift) to maximize raw execution speed, memory efficiency, and hardware vectorization.

## Core Value

Delivering high computational throughput and deterministic memory performance for mathematical algorithms with clean, modern syntax.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Lexer and tokenizer for mathematical expressions, identifiers, literals, and control flow
- [ ] Abstract Syntax Tree (AST) definitions and robust parser (Pratt / recursive descent)
- [ ] Semantic analysis and static type checker (type validation and inference)
- [ ] Intermediate Representation (IR) generation (targeting LLVM IR or Cranelift)
- [ ] Native binary code generation and driver CLI (`numlang compile`, `numlang run`)
- [ ] Core standard library for numerical computing (fixed-size arrays, vectors, math intrinsics)
- [ ] Benchmarking harness comparing execution speed against C and Rust baselines

### Out of Scope

- Exponential speedup over existing compiled languages across all benchmarks — Physical hardware limits (memory bandwidth, clock cycles, cache hierarchies) bound all software.
- Garbage collected runtime — Excluded to guarantee predictable latency and zero-cost abstractions.
- Dynamic typing and runtime interpretation — Excluded in favor of ahead-of-time (AOT) static optimization.

## Context

- **Implementation language**: Rust (utilizing algebraic data types, pattern matching, and memory safety for compiler frontend and IR transformations).
- **Compiler backend**: LLVM / Cranelift for machine-level optimization passes (SIMD auto-vectorization, loop unrolling, register allocation).
- **Target audience & use case**: Developers and researchers writing high-throughput mathematical simulations, numeric algorithms, and systems-level math code.

## Constraints

- **Tech stack**: Rust compiler toolchain (`cargo`, `rustc` 1.98+).
- **Platform**: Windows x86_64 native target, designed to be cross-platform.
- **Execution model**: Ahead-of-time compiled native binaries.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Implement compiler in Rust | Industry standard for compiler construction; powerful pattern matching and memory safety | — Pending |
| AOT compilation via LLVM/Cranelift | Leverages mature machine-level optimization pipelines rather than hand-crafting x86 backends | — Pending |
| Static typing with strict numeric types | Allows the compiler to generate optimal SIMD vector instructions without runtime checks | — Pending |

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
*Last updated: 2026-09-10 after initialization*
