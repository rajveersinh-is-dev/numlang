# NumLang Roadmap & Technical Architecture

## 1. High-Level Vision

NumLang is engineered to become a **genuinely state-of-the-art supercompiler**.
Unlike classical supercompilers (Turchin's Refal, Bolingbroke/Jones's GHC Supercompiler, SPSC, HOSC) which operate strictly on pure functional expression trees and suffer severe code blowup on state transitions, NumLang operates on **SSA Mid-Level IR (MIR)** with explicit heap modeling, polyhedral iteration spaces, and direct native code generation via Cranelift and LLVM.

All ongoing and future development adheres strictly to the [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md).

---

## 2. Milestone & Phase Tracker

### Completed Foundation (Phases 1–19)

| Phase | Description | Status | Commit / Artifact |
| :--- | :--- | :---: | :--- |
| **Phases 1–9** | **Core Language & AOT Compiler**<br>Lexer, Pratt parser, type system, Cranelift & LLVM backends, MemorySSA, Mem2Reg, BCE, SROA, formatter (`numlang fmt`), doc generator (`numlang doc`). | **Completed** | Full compiler v1.0, 40+ test suites |
| **Phase 10** | **Turchin Supercompilation & 1st Futamura**<br>AST ADTs, pattern matching, branch pruning, $O(N) \to O(1)$ recurrence solver, 1st Futamura interpreter specialization. | **Completed** | Commit `b22d64a` |
| **Phase 11** | **Higher-Order Functions & Closures**<br>Lambdas, environment capture, indirect calls, higher-order deforestation. | **Completed** | Commit `f540057` |
| **Phase 12** | **Polymorphic Types & Generics**<br>Type parameters `<T, U>`, monomorphization pass, generic standard library. | **Completed** | Commit `8463a6b` |
| **Phase 13** | **Heap Allocation & Pointer Driving**<br>`Box<T>`, `box`, `deref`, recursive data structures, symbolic heap. | **Completed** | Commit `585e096` |
| **Phase 14** | **Futamura Projections Prototype**<br>Interpreter specialization and VM simulation tests. | **Completed** | Commit `c3b5fcc` |
| **Phase 15** | **Differential Fuzzing & Lean 4 Setup**<br>100k differential fuzzing cases and initial Lean 4 proof environment. | **Completed** | Commits `e1362e3`, `52bc5e8` |
| **Phase 16** | **Canonical Academic Benchmark Suite**<br>Literature benchmarks, statistical runner, 95% bootstrap CIs. | **Completed** | Commit `d72fedb` |
| **Phase 17** | **Artifact Packaging & Paper Draft**<br>Docker hermetic environment, Zenodo metadata, PEPM paper draft. | **Completed** | Commit `e17c8e5` |
| **Phase 18** | **Distillation & MRSC Prototype**<br>Global process-tree transformation interface and multi-strategy CLI. | **Completed** | Commit `d068094` |
| **Phase 19** | **Polyhedral, Translation Validation & Parallel Driving Prototype**<br>Affine loop representation, symbolic equivalence checker, multi-threaded driving. | **Completed** | Commit `e83fb8b` |

---

### Remediation & Frontier Roadmap (Phases 20–40)

*Formulated to systematically resolve all findings from the comprehensive audit (`honest_review.md`).*

| Phase | Description | Status | Target Deliverables |
| :--- | :--- | :---: | :--- |
| **Phase 20** | **Fix Core Residualization, Knot Transfers & Textbook MSG**<br>Emit parallel state transfers on knot back-edges (`ProcessEdge::Knot`), remap Phi `BasicBlockId`s cleanly, replace curve-fitting with textbook anti-unification (Sørensen & Glück 1995). Eliminate all crashes on heap benchmarks (`nrev`, `append3`, `tree_flip`, `peano_mul`). | **Completed** | Commit `12c0867` |
| **Phase 21** | **Real Hamilton Global Distillation**<br>Replace DAG hash dedup with true Hamilton (2007) global distillation. Fold configurations across distinct function definitions and recursive call trees to deforest nested recursive compositions (e.g. `append3` in a single pass). | **Completed** | Commit `dc1b0ba` |
| **Phase 22** | **Real Multi-Result Supercompilation (MRSC)**<br>Replace 3-pass selector with true Mitchell & Klyuchnikov (2012) MRSC. Non-deterministic configuration hypergraph exploration with Pareto-optimal residual program extraction under user-defined cost metrics. | **Completed** | Commit `dea4cc3` |
| **Phase 23** | **Real Polyhedral Loop & Stencil Deforestation**<br>Implement affine iteration space polyhedra, dependence distance vectors, and legal loop fusion. Contract intermediate array memory buffers to $O(1)$ scalar temporaries or sliding windows. | **Completed** | Commit `5318b62` |
| **Phase 24** | **Formal SMT-Based Translation Validation**<br>Replace 256-step shallow testing with certified SMT translation validation. Extract Horn clauses and prove simulation preorder / bisimulation equivalence for all paths. | **Completed** | Commit `e7d3db1` |
| **Phase 25** | **Genuine Self-Applicable Specializer & 2nd and 3rd Futamura Projections**<br>Write `MinSpec.nl` in NumLang itself. Verify genuine 2nd projection ($\text{MinSpec}(\text{MinSpec}, \text{interp}) \to \text{compiler}$) and 3rd projection ($\text{MinSpec}(\text{MinSpec}, \text{MinSpec}) \to \text{cogen}$). Prove compiler generator idempotence. | **Completed** | Commit `81b2604` |
| **Phase 26** | **Rigorous Lean 4 Verification (Zero Axioms, Recursive Semantics)**<br>Eliminate `axiom kruskal_tree_theorem`. Extend semantics to model recursive functions, environments, and heap. Mechanize full semantic preservation proof without unproven axioms. | **Completed** | Commit `55fe450` |
| **Phase 27** | **Honest High-Precision Benchmarks & Supercompiler Comparisons**<br>Overhaul `runner.py` with in-process microsecond hardware timing ($N \ge 10,000$ iterations), zero process-spawn overhead, exit code validation, and direct head-to-head comparison against SPSC and HOSC on canonical literature benchmarks (KMP, Wadler deforestation, Peano, etc.). | **Completed** | Commit `fd030fa` |
| **Phase 28** | **Paper Rewrite & Reproducibility Package**<br>Rewrite `paper/main.tex` with automated cryptographic SHA-256 data pipeline, zero data fabrication, transparent reporting of speedups/neutral results, and 1-click Docker reproduction (`make reproduce`). | **Completed** | `paper/main.tex`<br>`bench/harness/generate_tables.py`<br>`docker/Dockerfile`<br>`Makefile` |
| **Phase 29** | **Fix Supercompiler Regressions**<br>Resolve 4 confirmed benchmark regressions: Ackermann (call-site depth budget & knot-tying), stream_fusion (loop-invariant call-site guards & no loop-inlining in loops), fib_matrix (MSG anti-unification fallback on recurrence failure), and power_spec (profitability gate bailing on zero reductions). Preserve all previous supercompiler wins. | **Completed** | Test suite & commit |
| **Phase 30** | **Supercompiler Refinements (MSG Knots, Zero-Edge Leaves, Inliner Loop Precomputation, Unified Gate)**<br>Materialize MSG generalized state as allocated knot target nodes; handle budget-exhaustion zero-edge non-return leaves safely with `Terminator::Unreachable`; precompute `has_loop_funcs` set in AST inliner to eliminate redundant full-AST scans; unify profitability gate across Classic, Distill, and MRSC modes. | **Completed** | Full suite green (100%), benchmarks stable |
| **Phase 31** | **Formal Termination Certificate & Order-3 Symbolic Recurrence**<br>Add machine-readable `TerminationWitness` recording HE whistle firings and header cutoffs; implement CLI `--emit-termination-proof` emitting JSON certificates; extend linear recurrence solver to symbolic trip counts for order-3 recurrences via `__order3_recurrence` intrinsic and native codegen lowering. | **Completed** | Full suite green (100%), benchmarks stable |
| **Phase 32** | **N-Way Mutual Recurrence Solver**<br>Add `NWayLinearSystem`, `detect_nway_linear_system` using integer Cramer's rule and fraction-free Gaussian elimination ($N \le 8$), binary matrix exponentiation `mat_pow_nxn`, and `solve_nway_recurrence` emitting `__nway_recurrence_i` intrinsics and exact closed forms; integrate into `SupercompilerDriver` loop recurrence pipeline. | **Completed** | Full suite green (100%), benchmarks stable |
| **Phase 33** | **Refinement Type Propagation Through Process Tree**<br>Add `Interval` arithmetic and `refinements` mapping to `SymbolicState`; interval propagation across arithmetic ops (`Add`, `Sub`, `Mul`); branch narrowing across 6 comparison operators; dead-branch pruning on empty intervals; call-site argument refinement propagation into inlined callees; supercompiler-level bounds-check elimination (`sc_bce_eliminated`). | **Completed** | Full suite green (100%), 5/5 new tests passing |
| **Phase 34** | **Full Higher-Order Closure Driving**<br>Add `SymTerm::ClosureVal(String, Vec<SymTermId>, Type)` for symbolic closures; track `Rvalue::ClosureAlloc` and `Rvalue::FnPtr` in `drive_statement`; drive through `Terminator::IndirectCall` in `drive_node` via `resolve_closure_function` and `try_drive_closure_call`; bind captured variables and call arguments into callee initial state with interval refinement propagation; fold closure invocations and deforest higher-order loops. | **Completed** | Full suite green (100%), 6/6 new tests passing |
| **Phase 35** | **Optimal Residual Code Size (Post-Distillation Compaction)**<br>Implement pre-residualization process-tree compaction (`compact_process_tree`) with dead overflow node elimination and alpha-equivalent knot deduplication; implement post-residualization MIR peephole pass (`compact_mir_function`) with identity assignment removal and conservative copy propagation (eta-reduction); add `residual_block_count` and `residual_stmt_count` to `SupercompilerStats` and `--supercompile-stats` CLI. | **Completed** | Full suite green (100%), 5/5 new tests passing |
| **Phase 36** | **Parallel Residualization (Independence Detection)**<br>Extend MIR with `Terminator::Fork { left, right, join }`; implement subtree/knot independence analysis (`ReadWriteSet`, `sets_are_independent`, `find_parallel_knot_pairs`) in `src/mir/supercompiler/independence.rs`; add `residualize_process_tree_parallel`; lower `Fork` in native backends with runtime shim `__numlang_fork_join`; expose opt-in `--parallel-residualize` CLI flag. | **Completed** | Full suite green (100%), 5/5 new tests passing |
| **Phase 37** | **Cross-Module Specialization Cache**<br>Add `SpecializationCache`, `CacheKey`, and `CachedSpecialization` in `src/mir/supercompiler/cache.rs` with content-addressed 2-level fanout JSON disk caching; derive `serde` across MIR AST; wire cache lookups and stores into `supercompile_mir_program_with_cache`; expose `--cache-dir` and `--no-cache` CLI flags. | **Completed** | Full suite green (100%), 5/5 new tests passing |
| **Phase 38** | **Full Mechanized Semantic Preservation Proof**<br>Machine-checked end-to-end semantic preservation in Lean 4 with zero unproven axioms and zero `sorry`; extended MIR operational semantics (`Semantics.lean`); Hamilton fold soundness (`Distillation.lean`); MRSC lattice selection soundness (`MRSC.lean`); refinement branch pruning soundness (`Refinement.lean`); compaction soundness (`Compaction.lean`); and end-to-end composition theorem `supercompiler_sound` (`Main.lean`). | **Completed** | 100% Lean verified (`lake build`), 5/5 new tests passing |
| **Phase 39** | **Real-World Benchmark Dominance (30-Benchmark Expansion & Head-to-Head Comparison)**<br>Expand benchmark suite from 13 to 30 programs across 5 domains (Pattern Matching, Sorting, Graph Algorithms, Numerical/Scientific Computing, Functional Idioms); extend `runner.py` with `BenchmarkEntry` taxonomy and empirical bootstrap CI harness; extend `generate_tables.py` with `generate_head_to_head_table()` and `generate_ablation_table()` emitting verifiable LaTeX tables; update `paper/main.tex` §7 Evaluation; fix polyhedral loop buffer contraction safety in `polyhedral.rs`. | **Completed** | Full suite green (100%), 5/5 new tests passing |


---

## 3. Architecture Deep-Dive: SSA Supercompiler Pipeline

```
                     Source (.nl)
                          │
                          ▼
                     Type Checker
                          │
                          ▼
                     MIR Lowering (`src/mir/lower.rs`)
                          │
                          ▼
            Mem2Reg & MemorySSA (`src/mir/mem2reg.rs`)
                          │
                          ▼
           SSA Supercompiler Engine (`src/mir/supercompiler/`)
             ├── Positive Symbolic Execution (`state.rs`, `term.rs`)
             ├── Topological Whistle & Embedding (`whistle.rs`)
             ├── Textbook Anti-Unification (MSG) (`generalize.rs`)
             ├── Hamilton Global Distillation (`distill.rs`)
             ├── MRSC Lattice Exploration (`mrsc.rs`)
             ├── Polyhedral Loop Fusion (`polyhedral.rs`)
             └── Knot State Transfer & Remapping (`residualize.rs`)
                          │
                          ▼
             SMT Translation Validation (`validate.rs`)
                          │
                          ▼
            Cranelift / LLVM Native Codegen
                          │
                          ▼
                 Native Standalone Binary (.exe / ELF)
```

---

## 4. Development Invariants & Integrity Mandate

1. **Academic Integrity**: Zero data tampering; all benchmark numbers generated directly from automated runs. All crashes logged transparently.
2. **Real Implementations**: Zero fake stubs; every module must implement the true mathematical algorithm from the literature.
3. **Formal Rigor**: Zero `sorry` and zero unproven `axiom` shortcuts in Lean 4 proofs.
4. **Pure Rust**: Core compiler builds portably via standard `cargo build` with zero required external C libraries.
5. **Clippy Clean**: Must pass `cargo clippy --all-targets -- -D warnings` with zero warnings.
6. **100% Green Tests**: All test suites must pass before committing changes.
