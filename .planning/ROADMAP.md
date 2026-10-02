# Roadmap: NumLang Compiler & Supercompiler

## Overview

NumLang combines Cranelift and LLVM code generation with a first-of-its-kind SSA Mid-level IR (MIR) supercompiler. This roadmap tracks all historical milestones and the active **Remediation & Frontier Milestone (Phases 20–28)** established to resolve all findings from [`honest_review.md`](file:///C:/Users/davea/.gemini/antigravity/brain/d21c1850-b996-4958-951d-ea34362faef0/honest_review.md).

---

## Historical Foundation (Phases 1–19) [COMPLETE]

### Phase 1: Core Language & AOT Compiler
- Lexer (`logos`), Pratt parser, type system, Cranelift AOT backend, basic optimizations.

### Phase 2: MemorySSA & Alias Analysis
- `src/mir/memory_ssa.rs`: `MemoryVersionId`, `MemoryDef`, `MemoryUse`, `MemoryPhi`.
- `src/mir/alias.rs`: Field-sensitive and array index alias analysis.
- `src/mir/mem2reg.rs`: Iterated dominance frontier promotion of memory places to pure SSA registers.

### Phases 10–13: Core Supercompiler & Language Extensions
- Phase 10: Turchin-style symbolic driving (`drive.rs`), homeomorphic embedding whistle (`whistle.rs`), AST interpreter specialization.
- Phase 11: Higher-order functions, lambdas, closures with environment capture, indirect calls.
- Phase 12: Generic type parameters `<T, U>`, monomorphization, generic standard library.
- Phase 13: Heap allocation (`Box<T>`, `box`, `deref`), symbolic heap in MIR driver.

### Phases 14–19: Advanced Tooling & Prototypes
- Phase 14: Futamura projections testing prototype.
- Phase 15: 100k differential fuzzing and initial Lean 4 proof environment.
- Phase 16: Canonical academic benchmark suite and statistical runner.
- Phase 17: Multi-stage Docker environment, Zenodo metadata, PEPM paper draft.
- Phase 18: Distillation and MRSC command-line interfaces.
- Phase 19: Polyhedral loop analysis, translation validation, and parallel driving (`--threads`).

---

## Remediation & Frontier Roadmap (Phases 20–28) [ACTIVE]

*Governed by [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md).*

### Phase 20: Fix Core Residualization, Knot Transfers & Textbook MSG [PLANNED]
- **Goal**: Fix crashing loop and heap binaries; replace curve-fitting heuristic with textbook anti-unification.
- **Scope**:
  - `src/mir/supercompiler/residualize.rs`: Emit parallel copies on knot back-edges (`ProcessEdge::Knot`); remap Phi incoming `BasicBlockId`s to residual blocks.
  - `src/mir/supercompiler/generalize.rs`: Implement textbook anti-unification (Sørensen & Glück 1995) for symbolic states.
- **Verification**: `nrev`, `append3`, `tree_flip`, `peano_mul` execute with zero crashes, exit code `0`, and exact output parity.

### Phase 21: Real Hamilton Global Distillation [PLANNED]
- **Goal**: Replace structural hash DAG dedup with genuine Hamilton (2007) global process-tree distillation.
- **Scope**:
  - `src/mir/supercompiler/distill.rs`: Implement global process-tree transformation, global whistle, and inter-procedural folding across recursive function definitions.
- **Verification**: Deforest nested recursive calls (`append(append(xs, ys), zs)`) into a single 3-argument function without intermediate heap allocations.

### Phase 22: Real Multi-Result Supercompilation (MRSC) [PLANNED]
- **Goal**: Replace 3-pass selector with true Mitchell & Klyuchnikov (2012) MRSC.
- **Scope**:
  - `src/mir/supercompiler/mrsc.rs`: Non-deterministic hypergraph configuration generator, branching on driving, folding, and generalization choices; Pareto-optimal residual program extraction.
- **Verification**: Automated discovery of Pareto-optimal configurations balancing code size and dynamic execution cost.

### Phase 23: Real Polyhedral Loop & Stencil Deforestation [PLANNED]
- **Goal**: Replace forward variable substitution with true polyhedral affine loop and stencil fusion.
- **Scope**:
  - `src/mir/supercompiler/polyhedral.rs`: Iteration domain extraction, access matrices, dependence distance vectors, legal loop fusion, and array buffer contraction.
- **Verification**: Multi-pass array stencils contract intermediate buffers to $O(1)$ scalar temporaries.

### Phase 24: Formal SMT-Based Translation Validation [PLANNED]
- **Goal**: Replace 256-step shallow testing with certified SMT-based translation validation.
- **Scope**:
  - `src/mir/supercompiler/validate.rs`: Verification Condition (VC) generation, QF_BV encoding, and SMT bisimulation proof over all CFG paths.
- **Verification**: Mathematical proof of simulation preorder emitted under `--verify-equivalence`.

### Phase 25: Genuine Self-Applicable Specializer & 2nd and 3rd Futamura Projections [PLANNED]
- **Goal**: Resolve the self-application impossibility by writing `MinSpec.nl` in NumLang itself.
- **Scope**:
  - `src/stdlib/minspec.nl`: Self-applicable partial evaluator in NumLang.
  - Execute and verify 1st ($\text{MinSpec}(\text{interp}, \text{prog})$), 2nd ($\text{MinSpec}(\text{MinSpec}, \text{interp})$), and 3rd ($\text{MinSpec}(\text{MinSpec}, \text{MinSpec})$) projections.
- **Verification**: $\text{cogen}(\text{interp})$ produces a standalone compiler that generates identical machine code.

### Phase 26: Rigorous Lean 4 Verification (Zero Axioms, Recursive Semantics) [PLANNED]
- **Goal**: Eliminate `axiom kruskal_tree_theorem` and extend semantics to model recursion and heap.
- **Scope**:
  - `proof/NumLangProofs/Semantics.lean`: Recursive function environments and heap pointers.
  - `proof/NumLangProofs/Termination.lean`: Constructive termination proof without unproven axioms.
  - `proof/NumLangProofs/Driving.lean`: Mechanized semantic preservation proof for driving, folding, and generalization.
- **Verification**: `lake build` passes with zero errors, zero warnings, zero `sorry`, and zero `axiom` declarations.

### Phase 27: Honest High-Precision Benchmarks & Supercompiler Comparisons [PLANNED]
- **Goal**: Overhaul benchmarking harness to eliminate OS process spawn artifacts; compare directly against SPSC and HOSC.
- **Scope**:
  - `bench/harness/runner.py`: In-process microsecond hardware performance counter timing ($N \ge 10,000$ iterations).
  - Bug fixes in C baselines (fix `append3` double-free).
  - Direct comparison against SPSC and HOSC on canonical literature benchmarks (KMP, Wadler deforestation, Peano multiplication, etc.).
- **Verification**: Automated script populates `bench/data/results.csv` with zero crashes and transparent comparison data.

### Phase 28: Paper Rewrite & Reproducibility Package [COMPLETE]
- **Goal**: Eliminate data fabrication in `paper/main.tex` and deliver a 1-click Docker reproduction package.
- **Scope**:
  - `paper/main.tex`: Full text and table revision backed by automated SHA-256 data pipeline.
  - `docker/Dockerfile`: Hermetic multi-stage build running `make reproduce`.
- **Verification**: Single command builds container and compiles `paper/main.pdf` with verified, reproducible figures and tables.

---

## Adversarial Remediation & System Soundness (Phases 41–45) [COMPLETE]

*Resolves all architectural debt, codegen monolith bloat, Win32/POSIX coupling, and constructive theorem verification.*

### Phase 41: Decouple Win32 & True POSIX Native Codegen [COMPLETE]
- **Goal**: Separate platform-specific linking and object emission; eliminate Win32 hardcoding.
- **Scope**: Platform-aware runtime linkers (`lld-link`, `cc`, `ld`), target triple discrimination, CI cross-compilation validation.
- **Verification**: Zero hardcoded `.lib` or Win32 imports in native Linux/Darwin codegen paths.

### Phase 42: Constructive Lean 4 Proof Modernization [COMPLETE]
- **Goal**: Fully modernize Lean 4 formal proofs for distillation, compaction, and semantic preservation with zero axioms and zero sorries.
- **Scope**: Complete proofs across `Semantics.lean`, `Distillation.lean`, `Compaction.lean`, `Preservation.lean`, and `Main.lean`.
- **Verification**: `lake build` passes with 0 errors, 0 axioms, and 0 warnings. Verified via `tests/constructive_lean4_phase42_tests.rs`.

### Phase 43: Complete Futamura Projections (1st, 2nd, 3rd) [COMPLETE]
- **Goal**: Rigorous verification of all three Futamura projections using `src/stdlib/minspec.nl`.
- **Scope**: 1st (specialization), 2nd (compiler generation), and 3rd (compiler-compiler generation / cogen).
- **Verification**: Rigorous test suites `tests/futamura_projection_tests.rs` and `tests/third_futamura_tests.rs` pass with zero regressions.

### Phase 44: Adversarial Fuzzing & Memory Safety Verification [COMPLETE]
- **Goal**: Eliminate memory leaks, address Sanitizer findings, and stress test supercompiler under adversarial inputs.
- **Scope**: Linear memory leak checks, Valgrind / ASan regression tests, fuzz-driven bug remediation.
- **Verification**: 100% green test suite across `tests/memory_leak_tests.rs` and related suites.

### Phase 45: Monolith Decomposition & Codegen Unification [COMPLETE]
- **Goal**: Partition Cranelift monolith into modular submodules under `src/codegen/cranelift/` (each <= 2,500 lines), abstract backend compilers, eliminate bare unwraps, purge deprecated AST supercompiler passes, and add inductive SMT loop validation ($k$-induction).
- **Scope**:
  - `src/codegen/cranelift/` (`mod.rs`, `abi.rs`, `intrinsics.rs`, `escape.rs`, `ast_stmt.rs`, `ast_expr.rs`, `mir_emit.rs`)
  - `BackendCompiler` trait unifying `CraneliftCompiler` and `LlvmCompiler`
  - $k$-induction loop translation validation in `src/mir/supercompiler/validate.rs`
  - Purged `src/opt/supercompiler/`
- **Verification**: All 77 test suites pass 100% green, `cargo clippy --all-targets -- -D warnings` reports 0 warnings.

