# Roadmap: NumLang Compiler & Supercompiler

## Overview

NumLang combines Cranelift and LLVM code generation with a first-of-its-kind SSA Mid-level IR (MIR) supercompiler. This roadmap tracks all historical milestones and the active **Hardening, Soundness & Architecture Remediation Milestone (Phases 41–45)** established to systematically resolve all findings from the comprehensive adversarial audit ([`ADVERSARIAL_AUDIT.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/ADVERSARIAL_AUDIT.md)).

---

## Historical Milestones

### Milestone 1: Core Foundation & Language Extensions (Phases 1–19) [COMPLETE]
- Lexer, Pratt parser, type system, Cranelift AOT backend, LLVM backend.
- MemorySSA, alias analysis, Mem2Reg IDF register promotion.
- Turchin supercompilation core, higher-order functions & closures, generics `<T, U>`, heap memory `Box<T>`.
- Differential fuzzing, canonical benchmark suite, and preliminary paper draft.

### Milestone 2: Remediation, Frontier & Stabilization (Phases 20–40) [COMPLETE]
- Core residualization knot transfers & textbook anti-unification (MSG).
- Hamilton global process-tree distillation & Mitchell/Klyuchnikov MRSC.
- Polyhedral stencil loop deforestation & SMT translation validation.
- Initial `MinSpec.nl` and Lean 4 formal semantics setup.
- High-precision in-process benchmarking harness (`runner.py`) and paper packaging.
- Supercompiler regressions fixed, MSG knot materialization, N-way mutual recurrence solver.
- Refinement type interval propagation, closure driving, code compaction, parallel residualization (`Fork`/`Join`), cross-module specialization cache, and 30-benchmark expansion.

---

## Active Milestone: Hardening, Soundness & Architecture (Phases 41–45) [PLANNED]

*Governed by [`INTEGRITY_RULES.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md).*

### Phase 41: Decouple Win32 & True POSIX Native Codegen [COMPLETE]
- **Goal**: Enable clean, native Linux/macOS compilation and ensure the Docker reproduction container executes without unresolved Windows kernel32 symbols.
- **Scope**:
  - `src/codegen/cranelift_backend.rs` & `src/codegen/llvm_backend.rs`: Abstract libc calls (`exit`, `write`, `malloc`). Only declare `ExitProcess`/`GetStdHandle`/`WriteFile` when `target_os = "windows"`.
  - `src/codegen/entry_bench.c`: Use standard ISO C `exit((int)ret);`.
  - `src/codegen/linker.rs`: Verify `link_unix` produces functional binaries with `cc obj.o -o exe -lm -no-pie`.
  - `docker/Dockerfile`: Verify `docker build` and `docker/entrypoint.sh` complete cleanly.
- **Verification**: Cross-platform tests pass on Linux without linker errors.

### Phase 42: Constructive Lean 4 Soundness Proofs [PLANNED]
- **Goal**: Replace vacuous tautological inductive constructors with genuine operational semantics and constructive preservation proofs.
- **Scope**:
  - `lean/Supercompiler/Semantics.lean`: Connect small-step `Step` to `Evaluates` via transitive closure $\text{Step}^*$. Fix `Fork` semantics.
  - `lean/Supercompiler/Compaction.lean`, `Refinement.lean`, `Main.lean`: Implement optimization passes as computable Lean definitions.
  - Eliminate constructor premises that assume `SemanticEquivalent f1 f2`.
  - Prove constructive simulation and soundness theorems with zero axioms and zero `sorry`.
- **Verification**: `lake build` in `lean/` compiles with zero errors, zero warnings, zero `sorry`, and zero unproven axioms.

### Phase 43: Authentic Self-Applicable Specializer & Futamura Projections [PLANNED]
- **Goal**: Replace the 3 identical copy-pasted functions in `minspec.nl` with a genuine self-applicable partial evaluator.
- **Scope**:
  - `src/stdlib/minspec.nl`: Implement a true partial evaluator `min_spec(prog, env)` capable of partially evaluating static operations while residualizing dynamic variables.
  - Implement genuine 2nd Futamura projection ($\text{MinSpec}(\text{MinSpec}, \text{interp})$).
  - Implement genuine 3rd Futamura projection ($\text{MinSpec}(\text{MinSpec}, \text{MinSpec})$).
  - Verify compiler generator idempotence and specialization correctness.
- **Verification**: `tests/true_futamura_projections_tests.rs` and `tests/third_futamura_tests.rs` pass with genuine self-application.

### Phase 44: Scoped Arena Allocator & Memory Safety [PLANNED]
- **Goal**: Eliminate monotonic memory leakage in heap, vector, and closure benchmarks by implementing a fast scoped bump-arena runtime.
- **Scope**:
  - `src/runtime/`: Implement scoped bump arena (`__nl_arena_create`, `__nl_arena_alloc`, `__nl_arena_reset`, `__nl_arena_destroy`).
  - `src/codegen/cranelift_backend.rs` & `src/codegen/llvm_backend.rs`: Lower `box(expr)` and closure environments to arena allocations.
  - Insert arena resets at loop headers and function boundaries for temporary objects.
  - Add memory residency profiling tests for `nrev`, `append3`, and iterative closure benchmarks.
- **Verification**: Memory profiling tests prove $O(1)$ peak heap residency during long-running loop execution.

### Phase 45: Architecture Decomposition, Codegen Unification & Hardening [PLANNED]
- **Goal**: Decompose monolithic source files, retire duplicate AST supercompiler, and harden arithmetic operations against panics/overflows.
- **Scope**:
  - Remove legacy AST supercompiler (`src/opt/supercompiler/`), unifying all optimizations onto SSA MIR.
  - Decompose `src/codegen/cranelift_backend.rs` (9,306 lines) into modular submodules (`abi.rs`, `builder.rs`, `intrinsics.rs`, `emit.rs`).
  - Standardize pipeline on `TypedAST -> MIR -> Cranelift/LLVM` and eliminate duplicate AST Cranelift codegen.
  - Replace raw `.unwrap()` calls with structured `CodegenError` and `TypeError` handling.
  - Fix unchecked arithmetic in `src/mir/supercompiler/generalize.rs` (`d * d`, `c1 + d`, precision loss in `disc as f64`).
- **Verification**: Full compiler builds cleanly with `cargo test --tests` (100% green) and 0 Clippy warnings under `-D warnings`.
