# NumLang Compiler

## What This Is

NumLang is a high-performance mathematical systems programming language and supercompiler targeting native x86-64 machine code via Cranelift and LLVM. It operates on an SSA Mid-level Intermediate Representation (MIR) and integrates symbolic execution, homeomorphic embedding (the whistle), generalization, distillation, multi-result supercompilation (MRSC), and polyhedral stencil loop fusion.

## Core Value

Achieve verifiable bare-metal performance, algebraic program transformation, and deforestation without compromises: **zero benchmark cheats, zero fake stubs, zero unproven formal axioms, and maximal scientific integrity.**

---

## Current Milestone: Hardening, Soundness & Architecture Remediation (Phases 41–45)

**Goal:** Transform NumLang from an academically vulnerable prototype with formal verification tautologies, broken cross-platform compilation, unbounded memory leaks, and pseudo-self-applicable Futamura projections into an indisputably sound, robust, and cleanly architected systems compiler.

**Origin:** Systematic remediation of all findings from the comprehensive adversarial audit in [`ADVERSARIAL_AUDIT.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/ADVERSARIAL_AUDIT.md).

**Target Outcomes:**
- **Phase 41**: Decouple Win32 & True POSIX Native Codegen (Linux/macOS support, Docker reproduction fix, libc abstraction).
- **Phase 42**: Constructive Lean 4 Soundness Proofs (Connect Step* to Evaluates, genuine semantic preservation without tautological constructors).
- **Phase 43**: Authentic Self-Applicable Specializer & Futamura Projections (Real partial evaluator in `MinSpec.nl`, genuine 2nd and 3rd projections).
- **Phase 44**: Scoped Arena Allocator & Memory Safety (Arena runtime, reset blocks, zero heap leaks in loops).
- **Phase 45**: Architecture Decomposition, Codegen Unification & Hardening (Retire AST supercompiler, break 9.3k LOC `cranelift_backend.rs`, eliminate unchecked unwrap/overflows).

---

## Requirements

### Validated (Completed Foundation: Phases 1–40)
- [x] **v1.0 Production Baseline (Phases 1–13)**: Lexer (`logos`), Pratt parser, type system, Cranelift & LLVM backends, MemorySSA, Mem2Reg, BCE, SROA, formatter (`numlang fmt`), doc generator (`numlang doc`), closures, generics, and initial heap primitives.
- [x] **Remediation & Frontier (Phases 20–28)**: Core residualization knot transfers, textbook MSG anti-unification, Hamilton global distillation, Mitchell & Klyuchnikov MRSC, polyhedral stencil deforestation, SMT-based translation validation, initial `MinSpec.nl`, hardware performance timing harness, and paper packaging.
- [x] **Stabilization & Frontier Extensions (Phases 29–40)**: Supercompiler regression fixes, MSG knot materialization, termination witnesses, N-way mutual recurrence solver, refinement type propagation, higher-order closure driving, residual code compaction, parallel residualization (`Fork`/`Join`), cross-module specialization cache, and 30 canonical literature benchmarks.

### Active (Hardening, Soundness & Architecture Remediation: Phases 41–45)
- [ ] **Phase 41**: Decouple Win32 & True POSIX Native Codegen (PORT-01..04).
- [ ] **Phase 42**: Constructive Lean 4 Soundness Proofs (LEAN-04..07).
- [ ] **Phase 43**: Authentic Self-Applicable Specializer & Futamura Projections (FUTA-05..08).
- [ ] **Phase 44**: Scoped Arena Allocator & Memory Safety (MEM-01..04).
- [ ] **Phase 45**: Architecture Decomposition, Codegen Unification & Hardening (ARCH-01..05).

---

## Out of Scope
- Dynamic runtime language reflection (NumLang is strictly AOT compiled).
- Full garbage collector with stop-the-world tracing (scoped arenas provide deterministic performance for numerical computing).
- Non-x86 architectures (ARM/RISC-V support deferred to post-v1.0 roadmap).

---

## Evolution & Governance
All changes strictly enforce [`INTEGRITY_RULES.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md) and address vulnerabilities documented in [`ADVERSARIAL_AUDIT.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/ADVERSARIAL_AUDIT.md).
