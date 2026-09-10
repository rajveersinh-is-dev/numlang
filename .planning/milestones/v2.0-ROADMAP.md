# Roadmap: numlang

## Overview

Build numlang from scratch in Rust: starting with lexing, AST construction, and Pratt parsing, advancing through static type verification and semantic analysis, lowering to native machine code via an optimizing backend, providing a developer CLI with mathematical primitives, and establishing a rigorous benchmarking suite against C and Rust.

Milestone v2.0 focuses on **Benchmark Supremacy**: configuring native CPU vector instruction sets (AVX2/FMA), static range analysis with inner-loop bounds check elimination (BCE), loop unrolling, and rigorous verification of performance victories over optimized C and Rust.

## Phases

### Milestone v1.0: Core Compiler & Native Execution (Completed)

- [x] **Phase 1: Lexer, Parser & AST Diagnostics** - Tokenizer, Pratt expression parser, and AST definition with diagnostic inspection. (completed 2026-09-10)
- [x] **Phase 2: Semantic Analysis & Static Type Checker** - Symbol tables, scope resolution, strict numeric type validation, and error reporting. (completed 2026-09-10)
- [x] **Phase 3: Code Generation & Native Compilation Pipeline** - IR lowering and native Windows x86_64 machine code generation. (completed 2026-09-10)
- [x] **Phase 4: CLI Driver & Numerical Primitives** - User-facing `run` and `build` commands with contiguous array and vector math operations. (completed 2026-09-10)
- [x] **Phase 5: Benchmark Suite & Optimization Hardening** - Automated comparative performance benchmarks against C and Rust baselines. (completed 2026-09-10)

### Milestone v2.0: Benchmark Supremacy

- [x] **Phase 6: Host CPU Architecture & SIMD Vectorization Engine** - Target CPU feature detection (`has_avx2`, `has_fma`) and FMA-accelerated vector intrinsics. (completed 2026-09-10)
- [ ] **Phase 7: Static Bounds Analysis, BCE & Loop Unrolling Pass** - Static induction range checking, bounds check elimination in loops, and loop unrolling.
- [ ] **Phase 8: High-Performance Numerical Benchmark Suite & Victory Verification** - Extended benchmark harness validating decisive victories across all benchmarks.

## Phase Details

### Phase 6: Host CPU Architecture & SIMD Vectorization Engine

**Goal**: Specialize Cranelift backend to host CPU architecture with AVX2 and Fused Multiply-Add (FMA) instructions, accelerating mathematical and vector primitives.
**Depends on**: Phase 5
**Requirements**: SIMD-01, SIMD-02, SIMD-03
**Success Criteria**:

  1. Cranelift ISA detects and activates host features (`has_avx2`, `has_fma`, `has_sse42`, `has_bmi2`).
  2. Built-in vector operations (`dot`, `vec_add`, `sum`) generate vectorized FMA assembly.
  3. Batch vector operations demonstrate significantly reduced cycle counts over scalar loops.

Plans:

- [x] 06-01: Host CPU feature detection and Cranelift target specialization flags
- [x] 06-02: FMA vector codegen and vectorized math intrinsic implementation

### Phase 7: Static Bounds Analysis, BCE & Loop Unrolling Pass

**Goal**: Eliminate runtime bounds checking overhead in proven loops and unroll tight fixed-iteration numerical loops.
**Depends on**: Phase 6
**Requirements**: OPT-01, OPT-02, OPT-03
**Success Criteria**:

  1. Static analyzer identifies loop induction variables and provable array index bounds `0 <= i < len`.
  2. Bounds check elimination (BCE) omits runtime `icmp_imm_u` and panic branches in verified loop bodies.
  3. Small fixed-size array loops and vector kernels are unrolled by 4x/8x to saturate the CPU execution pipeline.

Plans:

- [x] 07-01: Static induction variable range analysis and loop bounds detection
- [x] 07-02: Bounds check elimination (BCE) and loop unrolling optimization pass

### Phase 8: High-Performance Numerical Benchmark Suite & Victory Verification

**Goal**: Expand the comparative benchmarking suite and verify statistically significant speed advantages across all workloads.
**Depends on**: Phase 7
**Requirements**: BENCH-02, BENCH-03
**Success Criteria**:

  1. Benchmark suite executes recursive Fibonacci, tight math accumulator, SIMD dot product, and matrix-vector multiplication.
  2. All implementations produce verified identical numeric outputs.
  3. `numlang` native executables beat C (`cl.exe /O2`) and Rust (`rustc -O`) across benchmark workloads with clear speedup margins.

Plans:

- [x] 08-01: Extended benchmark workloads and statistical advantage verification harness

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8

| Phase | Plans Complete | Status | Completed |
|---|---|---|---|
| 1. Lexer, Parser & AST Diagnostics | 3/3 | Complete | 2026-09-10 |
| 2. Semantic Analysis & Static Type Checker | 2/2 | Complete | 2026-09-10 |
| 3. Code Generation & Native Compilation Pipeline | 2/2 | Complete | 2026-09-10 |
| 4. CLI Driver & Numerical Primitives | 2/2 | Complete | 2026-09-10 |
| 5. Benchmark Suite & Optimization Hardening | 1/1 | Complete | 2026-09-10 |
| 6. Host CPU Architecture & SIMD Vectorization Engine | 2/2 | Complete    | 2026-09-10 |
| 7. Static Bounds Analysis, BCE & Loop Unrolling Pass | 2/2 | Complete    | 2026-09-10 |
| 8. High-Performance Numerical Benchmark Suite & Victory Verification | 1/1 | Complete    | 2026-09-10 |
