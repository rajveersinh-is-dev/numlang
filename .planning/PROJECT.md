# NumLang Compiler

## What This Is

NumLang is a high-performance mathematical systems programming language and supercompiler targeting native x86-64 machine code via Cranelift and LLVM. It operates on an SSA Mid-level Intermediate Representation (MIR) and integrates symbolic execution, homeomorphic embedding (the whistle), generalization, distillation, multi-result supercompilation (MRSC), and polyhedral stencil loop fusion.

## Core Value

Achieve verifiable bare-metal performance, algebraic program transformation, and deforestation without compromises: **zero benchmark cheats, zero fake stubs, zero unproven formal axioms, and maximal scientific integrity.**

---

## Current Milestone: Remediation & Frontier (Phases 20–28)

**Goal:** Transform NumLang from a prototype with identified flaws into an indisputably verified, mathematically sound, world-class supercompiler by systematically resolving all 10 findings from [`honest_review.md`](file:///C:/Users/davea/.gemini/antigravity/brain/d21c1850-b996-4958-951d-ea34362faef0/honest_review.md) under [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md).

**Target Outcomes:**
- **Phase 20**: Fix `residualize.rs` knot-state transfers and Phi block remapping; implement textbook anti-unification (MSG) in `generalize.rs`. Eliminate crashes on heap benchmarks (`nrev`, `append3`, `tree_flip`, `peano_mul`).
- **Phase 21**: Real Hamilton (2007) global process-tree distillation in `distill.rs` to deforest nested recursive calls.
- **Phase 22**: Real Mitchell & Klyuchnikov (2012) Multi-Result Supercompilation (MRSC) exploring configuration hypergraphs with Pareto frontier selection.
- **Phase 23**: Real Polyhedral affine loop fusion and array buffer contraction in `polyhedral.rs`.
- **Phase 24**: Formal SMT-based translation validation in `validate.rs` proving simulation preorder over all execution paths.
- **Phase 25**: Genuine self-applicable `MinSpec.nl` in NumLang achieving authentic 2nd and 3rd Futamura projections with verified idempotence.
- **Phase 26**: Rigorous Lean 4 verification in `proof/`: eliminate `axiom kruskal_tree_theorem`, model recursive semantics, and prove soundness with zero `sorry` and zero axioms.
- **Phase 27**: Honest, in-process microsecond hardware benchmarking in `runner.py`, zero crashes, and direct head-to-head comparison against SPSC and HOSC.
- **Phase 28**: Full paper rewrite of `paper/main.tex` with automated SHA-256 data pipeline and 1-click Docker reproduction package.

---

## Requirements

### Validated (Completed Foundation)
- [x] **v1.0 Production Baseline**: Lexer (`logos`), Pratt parser, type system, Win32 Cranelift backend, LLVM backend, Monomorphization, BCE, SROA, formatter (`numlang fmt`), doc generator (`numlang doc`).
- [x] **v2.0 Phase 1**: SSA MIR CFG, basic block terminators, dominance analysis, and TypedAST lowering.
- [x] **v2.0 Phase 2**: MemorySSA (`MemoryDef`, `MemoryUse`, `MemoryPhi`), field-sensitive alias analysis, Mem2Reg IDF register promotion.
- [x] **v2.0 Phases 10–13**: Turchin supercompilation core (`drive.rs`, `whistle.rs`, `term.rs`, `state.rs`), closures and higher-order deforestation, generics `<T, U>`, and heap memory (`Box<T>`, `box`, `deref`).
- [x] **v2.0 Phases 14–19 Initial Prototypes**: Differential fuzzing (100k tests), benchmark suites, paper draft, and parallel driving (`--threads`).

### Active (Remediation & Frontier)
- [ ] **Phase 20**: Fix Residualization knot transfers & textbook MSG (RESID-01..04, MSG-01..03).
- [ ] **Phase 21**: Real Hamilton Global Distillation (DISTILL-01..03).
- [ ] **Phase 22**: Real Mitchell & Klyuchnikov MRSC (MRSC-01..03).
- [ ] **Phase 23**: Real Polyhedral Loop & Stencil Deforestation (POLY-01..03).
- [ ] **Phase 24**: SMT-Based Translation Validation (VALID-01..03).
- [ ] **Phase 25**: Genuine 2nd & 3rd Futamura Projections via `MinSpec.nl` (FUTA-01..04).
- [ ] **Phase 26**: Lean 4 Proofs without axioms over recursive semantics (LEAN-01..03).
- [ ] **Phase 27**: In-process microsecond benchmarks & SPSC/HOSC comparison (BENCH-01..04).
- [ ] **Phase 28**: Paper rewrite & 1-click Docker reproducibility bundle (PAPER-01..03).

---

## Out of Scope
- Dynamic runtime language reflection (NumLang is strictly AOT compiled).
- Raw unchecked pointer casting (memory safety is maintained through affine heap types).

---

## Evolution & Governance
All changes strictly enforce [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md). No data may be manually entered into paper tables; all results must be cryptographically hashed from automated execution.
