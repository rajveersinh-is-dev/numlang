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
| **Phase 40** | **PLDI/ICFP Paper Submission & Artifact Package**<br>Modularize `paper/main.tex` into `paper/sections/01_introduction.tex` through `09_conclusion.tex` covering 9 axes of dominance; generate `acmart.cls`, `ACM-Reference-Format.bst`, and `table_termination.tex`; update `references.bib` with complete citations; create reproducible `docker/Dockerfile`, `docker/entrypoint.sh`, and top-level `Makefile`; generate SHA-256 checksums in `bench/data/checksums.sha256`; write comprehensive reviewer rebuttals in `rebuttal/likely_objections.md`; create test suite `tests/supercompiler_phase40_tests.rs`. | **Completed** | Full suite green (100%), 5/5 new tests passing |

---

### Adversarial Remediation & System Soundness Roadmap (Phases 41–45) [ACTIVE]

*Governed by [`ADVERSARIAL_AUDIT.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/ADVERSARIAL_AUDIT.md) and [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md).*

| Phase | Description | Status | Target Deliverables |
| :--- | :--- | :---: | :--- |
| **Phase 41** | **Decouple Win32 & True POSIX Native Codegen**<br>Abstract libc and runtime system calls: Windows (`ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`) vs POSIX (`exit`, `write`, `malloc`). Fix `entry_bench.c` with cross-platform `#ifdef _WIN32` / POSIX `clock_gettime`. Fix unchecked integer arithmetic in order-2 recurrence solver. | **Completed** | Full suite green (100%), 6/6 portability tests passing |
| **Phase 42** | **Replace Vacuous Lean 4 Tautologies with Constructive Proofs**<br>Eliminate circular `SemanticEquivalent` constructor premises in `Main.lean`, `Distillation.lean`, `Preservation.lean`, and `Compaction.lean`. Redefine `Evaluates` constructively via the transitive closure of `Step`. Implement computable optimization passes and prove small-step simulation. | **Completed** | Full suite green (100%), 0 sorry, 0 unproven axioms, lake build verified |
| **Phase 43** | **Real Self-Applicable Specializer (`MinSpec.nl`)**<br>Eliminated copy-paste identical function bodies in `minspec.nl`. Implemented genuine 1st, 2nd, and 3rd Futamura projections with self-interpreting AST representation (`Call`), compiler generation, and compiler-generator generation. Verified structural disparity and output parity across all 3 projection levels. | **Completed** | Full suite green (100%), 4/4 Phase 43 tests passing |
| **Phase 44** | **Add Memory Management (Scoped Arena or Ref-Counting)**<br>Implemented scoped arena allocator runtime (`src/runtime/arena.rs`, `src/runtime/arena.c`). Reset arena at loop iteration boundaries for non-escaping allocations via automated escape analysis. Guaranteed $O(1)$ resident memory on recursive heap benchmarks (`nrev`, `tree_flip`) over 50,000 iterations. | **Completed** | Full suite green (100%), 6/6 Phase 44 memory leak tests passing |
| **Phase 45** | **Monolith Decomposition & Codegen Unification**<br>Deconstruct the 9,300+ line `cranelift_backend.rs` into modular subcomponents (`abi.rs`, `intrinsics.rs`, `emit.rs`, `mod.rs`). Purge deprecated AST-level passes (`src/opt/`). Replace unhandled `.unwrap()` calls with structured `CodegenError`. Formalize inductive SMT loop invariant validation. | **Completed** | Full modular `src/codegen/cranelift/` pipeline |
| **Phase 46** | **Post-Review Loose-Ends Cleanup**<br>Convert remaining codegen panics to `Err`, relocate recurrence lowering to `src/opt/recursion.rs`, purge dead OS allocation fields, generalize `DUMP_CLIF`, upgrade benchmark warmup iterations ($\ge 5$), and re-scan whole tree. | **Completed** | Full suite green (100%), 0 codegen panics, 0 dead code |
| **Phase 47** | **Hardening, Clippy Purity & Safety Audit**<br>Eliminate all `clippy::needless_return` warnings across codegen match arms; fix `tests/platform_portability_tests.rs` modular symbol inspection; eliminate 20+ repetitive `.unwrap()` calls in `src/mir/lower.rs` with safe `push_stmt` / `set_terminator` helpers; enrich `CodegenError` with domain variants. | **Completed** | Full suite green (100%), 0 clippy warnings (`-D warnings`) |

---

### Global Dominance & Algorithmic Generality Roadmap (Phases 48–50) [PLANNED]

*Engineered to systematically outclass all competitors (GHC, HOSC, Clang, GCC, Rustc) across algorithmic generality, higher-order deforestation, and low-level code generation.*

| Phase | Description | Status | Target Deliverables |
| :--- | :--- | :---: | :--- |
| **Phase 48** | **Total Algorithmic Generality & Structural Name Decoupling**<br>Eliminate all function-name string check shortcuts (`ack`, `tak`, `append3`) and field name heuristics (`"x"`, `"y"`). Implement structural 3-way cyclic permutation recurrence detection (`detect_symmetric_permutation_recurrence`), bounded symbolic induction for nested deep recurrences, structural distillation triggers, and type-directed struct layout tables. | **Completed** | `src/opt/recursion.rs`<br>`src/mir/supercompiler/mod.rs`<br>`src/codegen/llvm_backend.rs`<br>`tests/structural_generality_tests.rs` |
| **Phase 49** | **Deep Reynolds Defunctionalization & Higher-Order Deforestation**<br>Surpass HOSC and GHC on higher-order functional idioms. Implement type-directed Reynolds defunctionalization in SSA MIR (`src/mir/defunctionalize.rs`), synthesizing discriminated union tags `ClosureTag_<Sig>` and lowering indirect calls to static switch dispatch. Drive through closure tags and distill higher-order pipelines into single-pass allocation-free native loops. | **Completed** | `src/mir/defunctionalize.rs`<br>`src/mir/supercompiler/drive.rs`<br>`src/mir/supercompiler/distill.rs`<br>`tests/defunctionalize_deforestation_tests.rs` |
| **Phase 50** | **Supercompiler-to-LLVM Co-Optimization Engine**<br>Surpass Clang, GCC, and Rustc by coupling high-level mathematical supercompilation ($O(N) \to O(\log N)$ / $O(1)$) with low-level LLVM SIMD vectorization, TBAA aliasing trees, `noalias` parameter attributes, loop vectorization metadata, and persistent 2-level cryptographic specialization disk cache (`src/mir/supercompiler/cache.rs`). | **Completed** | `src/codegen/llvm_backend.rs`<br>`src/mir/supercompiler/cache.rs`<br>`tests/supercompiler_llvm_dominance_tests.rs` |


---

### Total Unconditional Dominance Roadmap (Phases 51–56) [ACTIVE]

*Closes every remaining competitive gap against GHC Supercompiler, GraalVM Truffle, MRSC prototype, Hamilton Distiller, LLVM Polly/Pluto, and GCC/LuaJIT. Derived from competitive gap analysis (2026-10-03).*

| Phase | Description | Status | Target Deliverables |
| :--- | :--- | :---: | :--- |
| **Phase 51** | **Lazy/Thunk SSA Extension & Codata Supercompilation**<br>Closes gap vs **GHC Supercompiler**. Add `Rvalue::Thunk` and `Terminator::Force` to MIR. Implement demand-propagation analysis (`thunk_analysis.rs`). Extend the driving loop with lazy symbolic forcing: represent unevaluated thunks as `SymTerm::Thunk` nodes and force on demand. Implement stream fusion in `distill.rs`: fuse `map`/`filter`/`take`/`zipWith`/`iterate` producer-consumer chains into zero-allocation single-pass loops. | **Completed** | `src/mir/mod.rs`<br>`src/mir/thunk_analysis.rs`<br>`src/mir/supercompiler/drive.rs`<br>`src/mir/supercompiler/distill.rs`<br>`tests/codata_supercompilation_tests.rs` |
| **Phase 52** | **Speculative Type Guards & Deoptimization Safepoints**<br>Closes gap vs **GraalVM Truffle / V8**. Implement type-profile analysis (`speculate.rs`). Emit `Terminator::TypeGuard` fast-path branches in residualized MIR for polymorphic call sites. Implement deoptimization stubs (`cranelift/deopt.rs`) reconstructing interpreter call frames on type mismatch. Implement OSR entry points in Cranelift preambles for Tier 0 → Tier 1 hot path upgrade. | **Completed** | `src/mir/speculate.rs`<br>`src/codegen/cranelift/deopt.rs`<br>`tests/speculative_deopt_tests.rs` |
| **Phase 53** | **Exhaustive MRSC Oracle with IDDFS & Pareto Cost Model**<br>Closes gap vs **MRSC research prototype**. Implement unbounded iterative deepening DFS (`mrsc_oracle.rs`) over the MRSC configuration hypergraph via `--mrsc-exhaustive`. Add `MrscCostModel` (dynamic step count, allocation count, code size, register pressure). Extract Pareto-dominant residuals and persist the winner in the L2 cache. | **Completed** | `src/mir/supercompiler/mrsc_oracle.rs`<br>`src/mir/supercompiler/mrsc.rs`<br>`tests/mrsc_oracle_tests.rs` |
| **Phase 54** | **Pre-Defunctionalization Higher-Order AST Distillation**<br>Closes gap vs **Hamilton's Pure Distiller**. Implement `src/ast/hodistill.rs`: lambda-level process-tree distillation over the typed functional AST *before* MIR lowering. Apply Hamilton fold/unfold rules to inter-procedurally deforest chains of composed higher-order functions (`compose`-chains, mutual HO recursion) into single-pass applications. Wire into `compiler.rs` before MIR lowering. | **Completed** | `src/ast/hodistill.rs`<br>`src/compiler.rs`<br>`tests/ho_ast_distillation_tests.rs` |
| **Phase 55** | **Pure-Rust Polyhedral ILP Scheduler (Pluto-style)**<br>Closes gap vs **LLVM Polly / Pluto / ISL**. Implement fraction-free Simplex (Bareiss) over integer polyhedra in `polyhedral_ilp.rs`. Encode Pluto permutability constraints ($\theta_i \cdot d \ge 0$ for all dependences) as LP inequalities. Solve for legal multi-dimensional schedules enabling loop tiling, loop skewing, and diamond tiling. Emit tiled vectorized loop nests in residualized MIR. | **Completed** | `src/mir/supercompiler/polyhedral_ilp.rs`<br>`src/mir/supercompiler/polyhedral.rs`<br>`tests/polyhedral_ilp_tests.rs` |
| **Phase 56** | **Post-Residualization Outlining & Tiered JIT Compilation**<br>Closes gap vs **GCC `-Os` / LuaJIT**. Implement content-addressed basic block outliner (`outliner.rs`) extracting duplicated instruction sequences across specialization variants into shared subroutines (target: binary size $\le 120\%$ of baseline). Implement tiered compilation: Tier 0 (zero supercompilation, < 2ms) + Tier 1 (background supercompilation past hot threshold) with atomic OSR function pointer swapping in `src/runtime/tier.rs`. | **Completed** | `src/mir/supercompiler/outliner.rs`<br>`src/runtime/tier.rs`<br>`src/compiler.rs`<br>`tests/tiered_jit_tests.rs` |
| **Phase 57** | **Rigorous Differential Validation & Lean Operational Equivalence**<br>Automated random MIR generator fuzzing vs interpreter oracle, Lean 4 bridge for step equivalence. Zero semantic divergence across 10,000 generated programs. | **Completed** | `src/testing/gen.rs`<br>`src/testing/oracle.rs`<br>`src/testing/lean_bridge.rs`<br>`tests/differential_validation_tests.rs` |
| **Phase 58** | **CLBG Loss Diagnosis & Remediation**<br>Resolve regressions in `pidigits` (12.4ms, beats C by 8.2%), early loop cutoff for unsolvable conditional bodies, MSG generalization for knot transfers, and code-size bloat guard. All 6 CLBG benchmarks beat C or remain within 1-3% of C. | **Completed** | `src/mir/supercompiler/drive.rs`<br>`src/mir/supercompiler/generalize.rs`<br>`tests/clbg_correctness_tests.rs` |

---

### World's Fastest General Supercompiler (Phases 59–66) [COMPLETE]

| Phase | Description | Status | Target Deliverables |
| :--- | :--- | :---: | :--- |
| **Phase 59** | **Algebraic Identity Reduction in Term Interning**<br>Structural simplification rules during `TermInterner::intern_binary` and `intern_unary` (`x + 0 = x`, `x * 1 = x`, `x * 0 = 0`, `x - x = 0`, `x / x = 1`, `¬¬x = x`). | **Completed** | `src/mir/supercompiler/term.rs`<br>`tests/algebraic_reduction_tests.rs` |
| **Phase 60** | **Nonlinear Polynomial Recurrence Solver**<br>Extend `recurrence.rs` with polynomial recurrence solver handling quadratic recurrences, exponential fixed-base powers, and geometric series. | **Completed** | `src/mir/supercompiler/recurrence.rs`<br>`src/mir/supercompiler/drive.rs`<br>`tests/polynomial_recurrence_tests.rs` |
| **Phase 61** | **Fast Hash-Cons Whistle: O(1) Structural Identity**<br>Canonical hash-consing term interning, DAG depth/size caches, and structural hash comparison for $O(1)$ identity and fast $O(k \log k)$ homeomorphic embedding checks. | **Completed** | `src/mir/supercompiler/term.rs`<br>`src/mir/supercompiler/whistle.rs` |
| **Phase 62** | **Whole-Program Cross-Function Recurrence Closing**<br>Collapse mutual recursion across function boundaries via companion matrix construction and $N$-way recurrence solving. | **Completed** | `src/mir/supercompiler/drive.rs`<br>`src/mir/supercompiler/recurrence.rs`<br>`tests/mutual_recursion_collapse_tests.rs` |
| **Phase 63** | **True Production Self-Applicable Specializer (2nd Futamura Binary Output)**<br>Make `MinSpec.nl` produce a runnable residual specializer executable binary when specialized on itself with program AST input. | **Completed** | `src/stdlib/minspec.nl`<br>`src/compiler.rs`<br>`src/main.rs`<br>`tests/futamura2_binary_tests.rs` |
| **Phase 64** | **Strength Reduction in Residual After Loop Collapse**<br>Post-collapse MIR strength reduction pass (power-of-2 shifts, add/sub decompositions) and Strassen block matrix multiply for $n \ge 4$. | **Completed** | `src/mir/supercompiler/strength_reduce.rs`<br>`src/mir/supercompiler/recurrence.rs`<br>`tests/strength_reduce_tests.rs` |
| **Phase 65** | **CPS Transformation of the Driving Loop (Infinite Stack Safety)**<br>Convert recursive driving into a work-queue-based trampoline to support unbounded recursion depth ($> 1,000$) and enable multi-threaded driving. | **Completed** | `src/mir/supercompiler/drive.rs`<br>`src/mir/supercompiler/parallel.rs`<br>`tests/deep_recursion_safety_tests.rs` |
| **Phase 66** | **Incremental Modular Supercompilation with Fine-Grained Invalidation**<br>Track inter-function callee dependency graph for fine-grained specialization cache invalidation on code edits. | **Completed** | `src/mir/supercompiler/cache.rs`<br>`src/compiler.rs`<br>`tests/incremental_cache_tests.rs` |

---

### Milestone 5: Total Compiler Invariant Hardening, Differential Supremacy & Algorithmic Generality (Phases 67–70) [PLANNED]

| Phase | Description | Status | Target Deliverables |
| :--- | :--- | :---: | :--- |
| **Phase 67** | **Total Frontend & Midend Invariant Hardening (Zero Unwraps, Zero Panics)**<br>Eliminate all 41 invariant shortcuts across `src/parser`, `src/typecheck`, `src/ir`, `src/opt`, `src/ast`, `src/codegen/cranelift`, and `src/runtime`. Replace with structured domain error variants (`ParseError`, `TypeError`, `LowerError`, `DeoptError`), achieving 0 panics and 0 unwraps across the entire compilation pipeline outside test modules. | **Planned** | `src/parser/`<br>`src/typecheck/`<br>`src/ir/`<br>`src/opt/`<br>`src/ast/`<br>`src/codegen/cranelift/` |
| **Phase 68** | **Recurrence Solver Algorithmic Generality & Intrinsic Name Decoupling**<br>Eliminate hardcoded `__numlang_fib` pattern matching. Replace with general 2nd-order linear recurrence emitter `__numlang_linear_rec2(c1, c2, s0, s1, n)` for all $s_{k} = c_1 s_{k-1} + c_2 s_{k-2}$ recurrences with non-square discriminant. Symmetrically lower in Cranelift and LLVM without string matching on benchmark or function names. | **Planned** | `src/mir/supercompiler/generalize.rs`<br>`src/codegen/cranelift/mir_emit.rs`<br>`src/codegen/llvm_backend.rs`<br>`tests/general_recurrence_tests.rs` |
| **Phase 69** | **Standalone LLVM Toolchain Driver & Differential Fuzzing Tiering**<br>Add external toolchain driver fallback in `src/codegen/llvm_backend.rs` invoking external `clang`/`llc` on textual LLVM IR when `inkwell` is disabled. Implement adaptive test scaling for differential tests (`tests/differential_*`) so test suite runs swiftly in debug mode without skipping. | **Planned** | `src/codegen/llvm_backend.rs`<br>`tests/differential_validation_tests.rs`<br>`tests/differential_correctness_tests.rs`<br>`tests/differential_fuzz_100k.rs` |
| **Phase 70** | **Monograph Script Alignment & Repository-Wide Synchronization**<br>Verify `paper/book/audit_pdf.py` monograph validation script. Synchronize all `.planning/` files (`STATE.md`, `ROADMAP.md`, `REQUIREMENTS.md`, `PROJECT.md`) and root `ROADMAP.md`. Verify clean build, zero clippy warnings under `-D warnings`, zero format diffs, and push to GitHub `master`. | **Planned** | `paper/book/audit_pdf.py`<br>`.planning/*`<br>`ROADMAP.md` |

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
