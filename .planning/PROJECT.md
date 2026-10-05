# NumLang Compiler

## What This Is

NumLang is a high-performance mathematical systems programming language and supercompiler targeting native x86-64 machine code via Cranelift and LLVM. It operates on an SSA Mid-level Intermediate Representation (MIR) and integrates symbolic execution, homeomorphic embedding (the whistle), generalization, distillation, multi-result supercompilation (MRSC), and polyhedral stencil loop fusion.

## Core Value

Achieve verifiable bare-metal performance, algebraic program transformation, and deforestation without compromises: **zero benchmark cheats, zero fake stubs, zero unproven formal axioms, and maximal scientific integrity.**

---

## Completed Milestones

### Remediation & Frontier (Phases 20–28) [COMPLETE]
Systematically resolved all 10 findings from [`honest_review.md`](file:///C:/Users/davea/.gemini/antigravity/brain/d21c1850-b996-4958-951d-ea34362faef0/honest_review.md) under [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md):
- Fixed residualization knot transfers & MSG (Phase 20)
- Real Hamilton global distillation (Phase 21)
- Real Mitchell & Klyuchnikov MRSC hypergraph search (Phase 22)
- Real Polyhedral loop & stencil deforestation (Phase 23)
- Formal SMT-based translation validation (Phase 24)
- Genuine self-applicable `MinSpec.nl` Futamura projections (Phase 25)
- Lean 4 verification without axioms (Phase 26)
- In-process microsecond hardware benchmarks (Phase 27)
- Paper rewrite and 1-click Docker reproduction package (Phase 28)

### Adversarial Remediation & System Soundness (Phases 41–45) [COMPLETE]
Systematically resolved all defect classes identified during adversarial audit [`ADVERSARIAL_AUDIT.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/ADVERSARIAL_AUDIT.md):
- Decoupled Win32 & implemented true POSIX native codegen (Phase 41)
- Replaced vacuous Lean 4 tautologies with constructive small-step `StepStar` proofs (Phase 42)
- Implemented real self-applicable specializer with distinct Futamura projections 1, 2, 3 (Phase 43)
- Implemented scoped arena allocator and non-escaping loop reset latch (Phase 44)
- Decomposed Cranelift monolith into 7 submodules (<= 2,500 lines each), unified backends with `BackendCompiler`, and added inductive SMT loop validation ($k$-induction) (Phase 45)

### Post-Review Polish & Hardening (Phases 46–47) [COMPLETE]
- Converted remaining codegen panics to structured errors, relocated recurrence lowering, purged dead OS alloc fields, hardened warmup iterations (Phase 46)
- Clippy -D warnings zero-warning purity, monadic MIR lowering unwrapping elimination, domain CodegenError variants (Phase 47)

### Global Dominance & Algorithmic Generality (Phases 48–50) [PLANNED]
Systematic architectural upgrades to outperform all functional (GHC, HOSC) and systems (GCC, Clang, Rustc) competitors:
- Total algorithmic generality and structural decoupling without name-matching heuristics (Phase 48)
- Whole-program type-directed Reynolds defunctionalization and higher-order loop deforestation (Phase 49)
- High-level mathematical supercompilation paired with low-level LLVM SIMD/TBAA co-optimization (Phase 50)

---

## Requirements

### Validated Foundation & Features
- [x] **v1.0 Production Baseline**: Lexer (`logos`), Pratt parser, type system, Win32 Cranelift backend, LLVM backend, Monomorphization, BCE, SROA, formatter (`numlang fmt`), doc generator (`numlang doc`).
- [x] **v2.0 Phase 1**: SSA MIR CFG, basic block terminators, dominance analysis, and TypedAST lowering.
- [x] **v2.0 Phase 2**: MemorySSA (`MemoryDef`, `MemoryUse`, `MemoryPhi`), field-sensitive alias analysis, Mem2Reg IDF register promotion.
- [x] **v2.0 Phases 10–13**: Turchin supercompilation core (`drive.rs`, `whistle.rs`, `term.rs`, `state.rs`), closures and higher-order deforestation, generics `<T, U>`, and heap memory (`Box<T>`, `box`, `deref`).
- [x] **v2.0 Phases 14–19 Initial Prototypes**: Differential fuzzing (100k tests), benchmark suites, paper draft, and parallel driving (`--threads`).
- [x] **Phase 20**: Fix Residualization knot transfers & textbook MSG (RESID-01..04, MSG-01..03).
- [x] **Phase 21**: Real Hamilton Global Distillation (DISTILL-01..03).
- [x] **Phase 22**: Real Mitchell & Klyuchnikov MRSC (MRSC-01..03).
- [x] **Phase 23**: Real Polyhedral Loop & Stencil Deforestation (POLY-01..03).
- [x] **Phase 24**: SMT-Based Translation Validation (VALID-01..03).
- [x] **Phase 25**: Genuine 2nd & 3rd Futamura Projections via `MinSpec.nl` (FUTA-01..04).
- [x] **Phase 26**: Lean 4 Proofs without axioms over recursive semantics (LEAN-01..03).
- [x] **Phase 27**: In-process microsecond benchmarks & SPSC/HOSC comparison (BENCH-01..04).
- [x] **Phase 28**: Paper rewrite & 1-click Docker reproducibility bundle (PAPER-01..03).
- [x] **Phase 41**: Cross-platform POSIX abstraction and safe recurrence arithmetic (PORT-01..05).
- [x] **Phase 42**: Constructive Lean 4 soundness proofs via `StepStar` operational semantics (LEAN-04..08).
- [x] **Phase 43**: Genuine expression-level self-applicable specializer and Futamura projections (FUTA-05..09).
- [x] **Phase 44**: Scoped arena allocator and loop-latch reset for $O(1)$ memory bounds (FUZZ-01..04).
- [x] **Phase 45**: Codegen decomposition into submodules $\le 2,500$ lines, `BackendCompiler` trait, and $k$-induction loop validation (CODEGEN-01..06).
- [x] **Phase 46**: Post-Review loose ends cleanup, zero codegen panics, zero dead code, $\ge 5$ warmup iterations (CLEAN-01..05).
- [x] **Phase 47**: Clippy warning elimination, platform test modernization, monadic MIR lowering unwrapping cleanup (HARDEN-01..05).
- [x] **Phase 48**: Total Algorithmic Generality & Structural Name Decoupling (GEN-01..05).
- [x] **Phase 49**: Deep Reynolds Defunctionalization & Higher-Order Deforestation (DEFUN-01..05).
- [x] **Phase 50**: Supercompiler-to-LLVM Co-Optimization Engine (COOPT-01..05).
- [x] **Phase 51**: Lazy/Thunk SSA Extension & Codata Supercompilation.
- [x] **Phase 52**: Speculative Type Guards & Deoptimization Safepoints.
- [x] **Phase 53**: Exhaustive MRSC Oracle with IDDFS & Pareto Cost Model.
- [x] **Phase 54**: Pre-Defunctionalization Higher-Order AST Distillation.
- [x] **Phase 55**: Pure-Rust Polyhedral ILP Scheduler.
- [x] **Phase 56**: Post-Residualization Outlining & Tiered JIT Compilation.
- [x] **Phase 57**: Rigorous Differential Validation & Lean Operational Equivalence.
- [x] **Phase 58**: CLBG Loss Diagnosis & Fix Plan.
- [x] **Phase 59**: Algebraic Identity Reduction in Term Interning (ALG-01..05).
- [ ] **Phase 60**: Nonlinear Polynomial Recurrence Solver (POLY-04..07).
- [ ] **Phase 61**: Fast Hash-Cons Whistle: O(1) Structural Identity (HASH-01..03).
- [ ] **Phase 62**: Whole-Program Cross-Function Recurrence Closing (XREC-01..04).
- [ ] **Phase 63**: True Production Self-Applicable Specializer (2nd Futamura Binary Output) (FUTA-10..13).
- [ ] **Phase 64**: Strength Reduction in Residual After Loop Collapse (STR-01..04).
- [ ] **Phase 65**: CPS Transformation of the Driving Loop (Infinite Stack Safety) (CPS-01..04).
- [ ] **Phase 66**: Incremental Modular Supercompilation with Fine-Grained Invalidation (INC-01..04).



---

## Out of Scope
- Dynamic runtime language reflection (NumLang is strictly AOT compiled).
- Raw unchecked pointer casting (memory safety is maintained through affine heap types).

---

## Evolution & Governance
All changes strictly enforce [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md). No data may be manually entered into paper tables; all results must be cryptographically hashed from automated execution.
