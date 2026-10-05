# NumLang: Complete Master Phase Prompts (Phases 1–45)

> **Document Purpose**: Single authoritative compendium of all execution prompts for the NumLang compiler and supercompiler from **Phase 1 through Phase 45**.  
> **Repository Root**: `C:\Users\davea\.gemini\antigravity\scratch\numlang`  
> **Governing Standards**: [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md) and [`ROADMAP.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/ROADMAP.md).

---

# Table of Contents

- [Part I: Production Compiler Baseline (Phases 1–9)](#part-i-production-compiler-baseline-phases-19)
  - [Phase 1: Master Refactoring & Baseline Setup](#phase-1-master-refactoring--baseline-setup)
  - [Phase 2: Zero-Warning Clippy Cleanliness](#phase-2-zero-warning-clippy-cleanliness)
  - [Phase 3: Core Control Flow (For Loops, Continue, Loop)](#phase-3-core-control-flow-for-loops-continue-loop)
  - [Phase 4: Standard I/O Built-ins (Print and Println)](#phase-4-standard-io-built-ins-print-and-println)
  - [Phase 5: Low-Bitwidth Signed Types (i8 and i16)](#phase-5-low-bitwidth-signed-types-i8-and-i16)
  - [Phase 6: Composite Struct Types & Field Access](#phase-6-composite-struct-types--field-access)
  - [Phase 7: Pattern Matching (Match Expressions)](#phase-7-pattern-matching-match-expressions)
  - [Phase 8: Production Diagnostic Polish & Explain CLI](#phase-8-production-diagnostic-polish--explain-cli)
  - [Phase 9: Built-in Standard Library (std)](#phase-9-built-in-standard-library-std)
- [Part II: Core Supercompiler & Language Extensions (Phases 10–19)](#part-ii-core-supercompiler--language-extensions-phases-1019)
  - [Phase 10: Turchin Supercompiler Core, ADTs & 1st Futamura Projection](#phase-10-turchin-supercompiler-core-adts--1st-futamura-projection)
  - [Phase 11: Higher-Order Functions, Closures & Pipeline Deforestation](#phase-11-higher-order-functions-closures--pipeline-deforestation)
  - [Phase 12: Polymorphic Types & Generics with Monomorphization](#phase-12-polymorphic-types--generics-with-monomorphization)
  - [Phase 13: Heap Allocation (Box<T>, Deref) & Symbolic Pointer Driving](#phase-13-heap-allocation-boxt-deref--symbolic-pointer-driving)
  - [Phase 14: Self-Applicable Specializer Prototype & Multistage Futamura](#phase-14-self-applicable-specializer-prototype--multistage-futamura)
  - [Phase 15: Differential Fuzzing (100k Cases) & Lean 4 Setup](#phase-15-differential-fuzzing-100k-cases--lean-4-setup)
  - [Phase 16: Canonical Literature Benchmarks & Statistical Harness](#phase-16-canonical-literature-benchmarks--statistical-harness)
  - [Phase 17: Multi-Stage Docker Artifact Packaging & PEPM Paper Draft](#phase-17-multi-stage-docker-artifact-packaging--pepm-paper-draft)
  - [Phase 18: Global Process-Tree Distillation & MRSC Interface](#phase-18-global-process-tree-distillation--mrsc-interface)
  - [Phase 19: Polyhedral Stencils, Translation Validation & Parallel Driving](#phase-19-polyhedral-stencils-translation-validation--parallel-driving)
- [Part III: Remediation & Frontier Supercompilation (Phases 20–28)](#part-iii-remediation--frontier-supercompilation-phases-2028)
  - [Phase 20: Fix Core Residualization, Back-Edge Knot Transfers & Textbook MSG](#phase-20-fix-core-residualization-back-edge-knot-transfers--textbook-msg)
  - [Phase 21: Real Hamilton Global Process-Tree Distillation](#phase-21-real-hamilton-global-process-tree-distillation)
  - [Phase 22: Real Multi-Result Supercompilation (MRSC) Hypergraph Search](#phase-22-real-multi-result-supercompilation-mrsc-hypergraph-search)
  - [Phase 23: Real Polyhedral Loop & Stencil Deforestation with Buffer Contraction](#phase-23-real-polyhedral-loop--stencil-deforestation-with-buffer-contraction)
  - [Phase 24: Formal SMT-Based Translation Validation & Simulation Preorder](#phase-24-formal-smt-based-translation-validation--simulation-preorder)
  - [Phase 25: Genuine Self-Applicable Specializer MinSpec.nl for 2nd and 3rd Futamura](#phase-25-genuine-self-applicable-specializer-minspecnl-for-2nd-and-3rd-futamura)
  - [Phase 26: Rigorous Lean 4 Formal Verification (Zero Axioms, Recursive Semantics)](#phase-26-rigorous-lean-4-formal-verification-zero-axioms-recursive-semantics)
  - [Phase 27: Honest High-Precision Benchmarks & Direct Supercompiler Comparisons](#phase-27-honest-high-precision-benchmarks--direct-supercompiler-comparisons)
  - [Phase 28: Paper Rewrite & 1-Click Reproducible Artifact Package](#phase-28-paper-rewrite--1-click-reproducible-artifact-package)
- [Part IV: Advanced Supercompilation, Parallelism & Rigorous Evaluation (Phases 29–40)](#part-iv-advanced-supercompilation-parallelism--rigorous-evaluation-phases-2940)
  - [Phase 29: Fix Supercompiler Regressions & Boundary Hardening](#phase-29-fix-supercompiler-regressions--boundary-hardening)
  - [Phase 30: Supercompiler Refinements (MSG Knots, Zero-Edge Leaves, Inliner Loop Precomputation, Unified Gate)](#phase-30-supercompiler-refinements-msg-knots-zero-edge-leaves-inliner-loop-precomputation-unified-gate)
  - [Phase 31: Formal Termination Certificate & Order-3 Symbolic Recurrence](#phase-31-formal-termination-certificate--order-3-symbolic-recurrence)
  - [Phase 32: N-Way Mutual Recurrence Solver (Linear System Solving & Matrix Exponentiation)](#phase-32-n-way-mutual-recurrence-solver-linear-system-solving--matrix-exponentiation)
  - [Phase 33: Refinement Type Propagation Through Process Tree](#phase-33-refinement-type-propagation-through-process-tree)
  - [Phase 34: Full Higher-Order Closure Driving](#phase-34-full-higher-order-closure-driving)
  - [Phase 35: Optimal Residual Code Size (Post-Distillation Compaction)](#phase-35-optimal-residual-code-size-post-distillation-compaction)
  - [Phase 36: Parallel Residualization (Independence Detection & Fork/Join Emitting)](#phase-36-parallel-residualization-independence-detection--forkjoin-emitting)
  - [Phase 37: Cross-Module Specialization Cache](#phase-37-cross-module-specialization-cache)
  - [Phase 38: Full Mechanized Semantic Preservation Proof in Lean 4](#phase-38-full-mechanized-semantic-preservation-proof-in-lean-4)
  - [Phase 39: Real-World Benchmark Dominance (30-Benchmark Expansion & Head-to-Head Comparison)](#phase-39-real-world-benchmark-dominance-30-benchmark-expansion--head-to-head-comparison)
  - [Phase 40: PLDI/ICFP Paper Submission & Artifact Package](#phase-40-pldiicfp-paper-submission--artifact-package)
- [Part V: Adversarial Remediation & System Soundness (Phases 41–45)](#part-v-adversarial-remediation--system-soundness-phases-4145)
  - [Phase 41: Decouple Win32 & Implement True POSIX Native Codegen](#phase-41-decouple-win32--implement-true-posix-native-codegen)
  - [Phase 42: Replace Vacuous Lean 4 Tautologies with Constructive Proofs](#phase-42-replace-vacuous-lean-4-tautologies-with-constructive-proofs)
  - [Phase 43: Implement Real Self-Applicable Specializer (MinSpec.nl)](#phase-43-implement-real-self-applicable-specializer-minspecnl)
  - [Phase 44: Add Memory Management (Scoped Arena or Ref-Counting)](#phase-44-add-memory-management-scoped-arena-or-ref-counting)
  - [Phase 45: Monolith Decomposition & Codegen Unification](#phase-45-monolith-decomposition--codegen-unification)

---

# Part I: Production Compiler Baseline (Phases 1–9)

---

## Phase 1: Master Refactoring & Baseline Setup

### Objective
Establish the clean production baseline of NumLang. Purge all historical ad-hoc hacks, hardcoded benchmark shortcuts, and unverified elevation passes, leaving an honest Pratt-parser, type-checking, and Cranelift backend pipeline.

### Target Files
- `src/token.rs`, `src/ast.rs`, `src/parser/`
- `src/typecheck/checker.rs`, `src/typecheck/types.rs`
- `src/codegen/cranelift_backend.rs`, `src/main.rs`
- Delete: `entry_bench.c`, `math_elevation.rs`

### Technical Tasks
1. Remove all manual pattern matching that hardcodes specific benchmark outputs.
2. Ensure integer promotions, suffixed literals (`100u32`, `200i64`), and bitwise operations obey standard twos-complement and unsigned modular arithmetic.
3. Clean up the command-line interface in `src/main.rs` (`numlang build`, `numlang run`).
4. Ensure cross-platform linking uses `rust-lld` on Windows or system `cc`/`clang` on Unix.

### Verification Gate
- `cargo build` compiles without errors.
- `cargo test --tests` passes 100% green.

---

## Phase 2: Zero-Warning Clippy Cleanliness

### Objective
Eliminate every single Clippy warning across all compiler modules, test suites, and binaries under `-- -D warnings`.

### Target Files
- `src/codegen/cranelift_backend.rs`
- `src/opt/bce.rs`, `src/opt/inlining.rs`, `src/opt/loop_opt.rs`, `src/opt/while_unroll.rs`
- `tests/multi_language_benchmarks.rs`, `tests/benchmark_harness.rs`

### Technical Tasks
1. Refactor `too_many_arguments` functions into structured context structs (e.g. `SpecializeCtx`).
2. Replace manual checked arithmetic with standard checked operations (`checked_div`, `checked_mul`).
3. Replace manual range checks with `.contains(&x)` and collapsible matches with unified patterns.
4. Eliminate needless borrows and generic slice coercions.

### Verification Gate
- `cargo clippy --all-targets -- -D warnings` terminates with zero warnings and zero errors.

---

## Phase 3: Core Control Flow (For Loops, Continue, Loop)

### Objective
Expand NumLang's control flow syntax from simple `while` loops to include `for .. in` range loops, `continue`, and infinite `loop` blocks.

### Target Files
- `src/token.rs`: Add `For`, `In`, `Continue`, `Loop`, `DotDot` (`..`), `DotDotEq` (`..=`).
- `src/ast.rs`, `src/parser/stmt.rs`: Parse `Stmt::For`, `Stmt::Continue`, `Stmt::Loop`.
- `src/typecheck/checker.rs`: Type-check range bounds, track `loop_depth`, prevent `continue` outside loops.
- `src/ir/lower.rs`: Desugar `Stmt::For` into canonical while loops with induction increments.
- `src/codegen/cranelift_backend.rs`: Handle `continue` jumps to the active loop header.

### Verification Gate
- Add `tests/control_flow_tests.rs`.
- Verify range iteration sums, inclusive bounds, even-number skipping via `continue`, and `loop { break; }`.

---

## Phase 4: Standard I/O Built-ins (Print and Println)

### Objective
Implement built-in `print` and `println` intrinsics supporting formatted console output for integers, floats, booleans, and string constants without requiring external libc dependencies.

### Target Files
- `src/token.rs`: Add `StringLiteral` regex token.
- `src/ast.rs`, `src/parser/expr.rs`: Add string literals in AST.
- `src/typecheck/checker.rs`: Register `print` and `println` built-ins accepting primitive arguments or string literals.
- `src/codegen/cranelift_backend.rs`: Emit in-memory itoa routines on the stack and invoke native Win32 `WriteFile` on `STD_OUTPUT_HANDLE` (or Unix `write(1)`).

### Verification Gate
- Add `tests/io_tests.rs`.
- Verify stdout capture for `"Hello World"`, negative integers, and booleans.

---

## Phase 5: Low-Bitwidth Signed Types (i8 and i16)

### Objective
Complete the integer type matrix by introducing signed 8-bit (`i8`) and signed 16-bit (`i16`) integers alongside the existing unsigned types.

### Target Files
- `src/typecheck/types.rs`: Add `Type::I8`, `Type::I16`, updating size, alignment, and display formatting.
- `src/token.rs`: Extend `TypedIntLiteral` regex to accept `i8` and `i16` suffixes.
- `src/codegen/cranelift_backend.rs`: Map `Type::I8 => types::I8` and `Type::I16 => types::I16`.

### Verification Gate
- Verify wrapping arithmetic, signed sign-extension, and bounds in `tests/unsigned_type_tests.rs`.

---

## Phase 6: Composite Struct Types & Field Access

### Objective
Implement flat stack-allocated C-style structs, named struct definitions, struct literal instantiation, and member access (`p.x`).

### Target Files
- `src/token.rs`: Add `Struct`, `Dot`.
- `src/ast.rs`, `src/parser/`: Parse top-level `struct Name { field: Type }`, struct literals `Name { x: 1 }`, and field access `p.x`.
- `src/typecheck/checker.rs`: Implement struct symbol table, field offset calculation, and type checking for field reads/writes.
- `src/codegen/cranelift_backend.rs`: Allocate aligned stack slots and emit memory loads/stores at computed byte offsets.

### Verification Gate
- Add `tests/struct_tests.rs`.
- Verify passing structs to functions by value and reading/writing nested fields.

---

## Phase 7: Pattern Matching (Match Expressions)

### Objective
Introduce first-class pattern matching over integers and booleans with exhaustive checking and or-patterns (`0 | 1 => ...`).

### Target Files
- `src/token.rs`: Add `Match`, `FatArrow` (`=>`), `Pipe` (`|`), `Underscore` (`_`).
- `src/ast.rs`, `src/parser/expr.rs`: Parse `Expr::Match` with pattern arms.
- `src/typecheck/checker.rs`: Validate pattern arm types against scrutinee; verify arm body type consistency.
- `src/codegen/cranelift_backend.rs`: Lower matches to multi-way branch comparisons and phi merges.

### Verification Gate
- Add `tests/match_tests.rs`.
- Verify integer dispatch, wildcard arms, and conditional expressions.

---

## Phase 8: Production Diagnostic Polish & Explain CLI

### Objective
Integrate `miette` span-highlighted reporting across all syntax and semantic errors, adding actionable suggestions and an interactive `--explain <CODE>` CLI.

### Target Files
- `src/diagnostic.rs`: Add error codes `E001` through `E020`.
- `src/typecheck/checker.rs`: Attach exact spans and contextual secondary labels to all `TypeError` variants.
- `src/main.rs`: Wire `numlang --explain <CODE>`.

### Verification Gate
- Add `tests/diagnostics_tests.rs`.
- Verify stderr formatting, line numbers, source snippets, and help text.

---

## Phase 9: Built-in Standard Library (std)

### Objective
Provide built-in mathematical intrinsics, bitwise utilities, and type conversion primitives directly linked into Cranelift code generation.

### Target Files
- `src/typecheck/checker.rs`, `src/codegen/cranelift_backend.rs`
- Add: `sqrt`, `abs`, `min`, `max`, `popcnt`, `clz`, `ctz`, `bswap`, and numeric casts (`i64_to_f64`, `f64_to_i64`).
- Link external math functions (`sin`, `cos`, `tan`, `ln`, `exp`) from system runtime.

### Verification Gate
- Add `tests/stdlib_tests.rs`.
- Verify accurate floating-point and bit-manipulation output across all primitives.

---

# Part II: Core Supercompiler & Language Extensions (Phases 10–19)

---

## Phase 10: Turchin Supercompiler Core, ADTs & 1st Futamura Projection

### Objective
Implement the foundational Turchin-style symbolic driving engine, algebraic data types (enums), pattern-based branch pruning, closed-form recurrence solver ($O(N) \to O(1)$), and verified 1st Futamura projection.

### Target Files
- `src/mir/supercompiler/drive.rs`: Symbolic state propagation and unfolding.
- `src/mir/supercompiler/whistle.rs`: Homeomorphic embedding whistle based on Kruskal's Tree Theorem.
- `src/mir/supercompiler/generalize.rs`: Recurrence detection and closed-form polynomial solver.
- `src/mir/supercompiler/residualize.rs`: Residual MIR generation.

### Technical Tasks
1. Drive functions symbolically with `Value::Symbolic` inputs.
2. Implement branch pruning: when a condition evaluates under current path constraints, prune the dead branch.
3. Formulate the 1st Futamura projection: specialize a self-contained NumLang interpreter on a static program, verifying complete elimination of interpreter dispatch overhead.

### Verification Gate
- `tests/supercompiler_symbolic_tests.rs` passes.
- `tests/futamura_projection_tests.rs` validates interpreter dispatch collapse to straight-line code.

---

## Phase 11: Higher-Order Functions, Closures & Pipeline Deforestation

### Objective
Introduce first-class functions, anonymous lambdas, closure environment capture, indirect calls, and higher-order pipeline deforestation.

### Target Files
- `src/token.rs`: Add `Fn` types, lambda syntax `|x| ...`.
- `src/typecheck/types.rs`: Add `Type::Fn(Vec<Type>, Box<Type>)` and `Type::Closure`.
- `src/mir/lower.rs`: Lower closures to fat pointers `(fn_ptr, env_ptr)`.
- `src/mir/supercompiler/drive.rs`: Inline and drive higher-order calls, eliminating intermediate pipeline allocations.

### Verification Gate
- Add `tests/higher_order_tests.rs`.
- Verify `map(f, filter(g, xs))` fuses to a single pass with zero intermediate array allocation.

---

## Phase 12: Polymorphic Types & Generics with Monomorphization

### Objective
Implement generic functions and data structures with explicit type parameters `<T, U>`, supported by a whole-program monomorphization pass.

### Target Files
- `src/ast.rs`, `src/typecheck/types.rs`: Add `Type::Param(String)`.
- `src/typecheck/checker.rs`: Type-check generic definitions and instantiate concrete specializations.
- `src/opt/monomorphize.rs`: Duplicate and specialize all generic function bodies prior to MIR lowering.

### Verification Gate
- Add `tests/generics_tests.rs`.
- Verify generic container implementations (`map<T, U>`, `fold<T, A>`) compile to zero-overhead monomorphic machine code.

---

## Phase 13: Heap Allocation (Box<T>, Deref) & Symbolic Pointer Driving

### Objective
Add safe heap memory primitives (`Box<T>`, `box`, `deref`), dynamic allocation, and symbolic heap modeling within the supercompiler driver.

### Target Files
- `src/typecheck/types.rs`: Add `Type::Box(Box<Type>)`.
- `src/mir/lower.rs`: Add heap allocation statements and load/store dereferencing.
- `src/mir/supercompiler/state.rs`: Model a symbolic heap mapping abstract pointer IDs to symbolic terms.

### Verification Gate
- Add `tests/heap_tests.rs`.
- Verify linked list and binary tree construction, recursive traversal, and heap dereference.

---

## Phase 14: Self-Applicable Specializer Prototype & Multistage Futamura

### Objective
Prototype a specialized interpreter engine (`MinSpec`) capable of processing symbolic syntax trees, and evaluate the foundations of multistage Futamura projections.

### Target Files
- `src/stdlib/meta.nl`: Abstract syntax tree definition and meta-evaluator in NumLang.
- `tests/third_futamura_tests.rs`: Multistage specialization test harness.

### Verification Gate
- Verify that specializing `meta.nl` against static inputs eliminates interpretation loops.

---

## Phase 15: Differential Fuzzing (100k Cases) & Lean 4 Setup

### Objective
Build a differential fuzz testing engine generating 100,000 synthetic programs to verify semantic equivalence between the supercompiler and the reference interpreter; initialize the Lean 4 formal proof repository.

### Target Files
- `fuzz/fuzz_engine.rs`: AST fuzzer generating arithmetic, branching, and loop programs.
- `proof/NumLangProofs/Semantics.lean`: Formal operational semantics scaffold.
- `proof/lakefile.lean`: Lean 4 project configuration.

### Verification Gate
- Run 100,000 differential fuzz test cases with zero discrepancies.
- `lake build` compiles the Lean proof directory.

---

## Phase 16: Canonical Literature Benchmarks & Statistical Harness

### Objective
Assemble 10 canonical supercompiler benchmarks from the literature, implemented across NumLang, Rust, C, and Haskell (GHC), accompanied by a statistical harness calculating 95% bootstrap confidence intervals.

### Target Files
- `bench/benchmarks/`: `nrev`, `append3`, `stream_fusion`, `ackermann`, `fib_matrix`, `sieve`, `matvec_4x4`, `raytracer_sphere`, `tree_flip`, `peano_mul`.
- `bench/harness/runner.py`: Execution script with CPU affinity pinning and statistical analysis.

### Verification Gate
- Automated benchmark execution generates raw metrics in `bench/data/results.csv`.

---

## Phase 17: Multi-Stage Docker Artifact Packaging & PEPM Paper Draft

### Objective
Package a hermetic Docker container and draft a full ACM SIGPLAN PEPM research paper detailing the SSA supercompiler architecture.

### Target Files
- `docker/Dockerfile`: Multi-stage build bundling Rust, Clang, GHC, Python, and Lean 4.
- `paper/main.tex`: 12-page research paper draft with architecture diagrams and evaluation sections.
- `REPRODUCIBILITY.md`, `LICENSE`, `.zenodo.json`.

### Verification Gate
- `docker build` succeeds without networking dependencies after caching.

---

## Phase 18: Global Process-Tree Distillation & MRSC Interface

### Objective
Implement the command-line flags and architectural interfaces for global process-tree transformation (Hamilton Distillation) and Multi-Result Supercompilation (MRSC).

### Target Files
- `src/mir/supercompiler/distill.rs`: Distillation interface.
- `src/mir/supercompiler/mrsc.rs`: MRSC search interface.
- `src/main.rs`: Wire `--mode <classic|distill|mrsc>` and `--mrsc-objective <size|branch|pareto>`.

### Verification Gate
- Unit tests verify CLI dispatch to selected optimization modes.

---

## Phase 19: Polyhedral Stencils, Translation Validation & Parallel Driving

### Objective
Implement polyhedral array loop representation, symbolic execution translation validation, and multi-threaded scoped parallel driving.

### Target Files
- `src/mir/supercompiler/polyhedral.rs`: Affine loop representation.
- `src/mir/supercompiler/validate.rs`: Bounded translation validator (`--verify-equivalence`).
- `src/mir/supercompiler/parallel.rs`: Scoped multi-threaded driving (`--threads <N>`).

### Verification Gate
- Verify multi-threaded compilation speedup on multi-core hardware.
- Verify translation validation flags synthetic semantic corruptions.

---

# Part III: Remediation & Frontier Supercompilation (Phases 20–28)

*Governed by [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md) — All implementations must be mathematically genuine with zero fabricated data, zero fake stubs, and zero unproven axioms.*

---

## Phase 20: Fix Core Residualization, Back-Edge Knot Transfers & Textbook MSG

### Context & Problem Statement
The independent audit in `honest_review.md` revealed that when `residualize.rs` encounters a knot back-edge (`ProcessEdge::Knot(anc_id)`), it emits a jump without generating parallel copy state transfers, leaving loop induction variables un-updated. Furthermore, `SymTerm::Phi` nodes retain the stale `BasicBlockId`s of the original unoptimized program. Consequently, all heap-allocating and recursive benchmarks (`nrev`, `append3`, `tree_flip`, `peano_mul`) crash with stack overflows or segfaults (`STATUS_STACK_OVERFLOW: -1073741571`). Additionally, `generalize.rs` relies on an ad-hoc polynomial curve fitter rather than textbook Most-Specific Generalization.

### Target Files
- `src/mir/supercompiler/residualize.rs`
- `src/mir/supercompiler/generalize.rs`
- `tests/heap_supercompile_tests.rs`

### Technical Tasks
1. **Parallel Copy Knot State Transfer**:
   - Compare the active environment $\sigma_{\text{curr}}$ at the knot node with the ancestor environment $\sigma_{\text{anc}}$.
   - Compute the substitution $\theta = \{ x \mapsto t_x \mid x \in \text{dom}(\sigma_{\text{anc}}) \}$.
   - Emit parallel copy assignments (resolving any cyclic dependencies via temporary scratch variables) before emitting `Terminator::Branch { target: node_to_block[anc_id] }`.
2. **Phi Node Residual Block Remapping**:
   - Construct a mapping from process tree transitions to residual `BasicBlockId`s: `(ProcessNodeId, ProcessNodeId) -> BasicBlockId`.
   - Update `SymTerm::Phi` emission to rewrite incoming predecessor block IDs to their residual counterparts.
3. **Textbook First-Order Anti-Unification (MSG)**:
   - In `generalize.rs`, implement standard anti-unification (Sørensen & Glück 1995; Plotkin 1970).
   - Compute $\text{msg}(t_1, t_2) = (t_0, \theta_1, \theta_2)$ where $t_0 \theta_1 = t_1$ and $t_0 \theta_2 = t_2$, minimizing variable introductions.
   - Generalize symbolic environments component-wise when the whistle triggers.
4. **Integration Verification**:
   - Compile `nrev.nl`, `append3.nl`, `tree_flip.nl`, and `peano_mul.nl` with `--supercompile`.
   - Verify exit code `0` and identical output compared to baseline unoptimized execution.

### Verification Gate
- `cargo test --tests` passes 100% green.
- `nrev`, `append3`, `tree_flip`, and `peano_mul` execute with **0 crashes** and **sub-millisecond runtime**.

---

## Phase 21: Real Hamilton Global Process-Tree Distillation

### Context & Problem Statement
`src/mir/supercompiler/distill.rs` currently implements a trivial DAG hash deduplication stub, misrepresenting Hamilton's (2007) global distillation. Real distillation operates globally across the entire process tree to fold configurations that supercompilation cannot fold, eliminating intermediate recursive data structures.

### Target Files
- `src/mir/supercompiler/distill.rs`
- `tests/distillation_tests.rs`

### Technical Tasks
1. **Global Process Tree Representation**:
   - Extend the process tree to represent cross-procedural configurations and call graph unfoldings globally.
2. **Two-Level Whistle & Global Knot-Tying**:
   - Implement the global whistle comparing configurations across distinct recursive function scopes.
   - When a global configuration embeds into an ancestor, perform global generalization and extract new specialized recursive functions.
3. **Deforestation of Composed Recursive Structures**:
   - Deforest compositions such as `append(append(xs, ys), zs)` into a single 3-argument function `append3(xs, ys, zs)` that recurses directly without intermediate list construction.

### Verification Gate
- `tests/distillation_tests.rs` proves that `append(append(xs, ys), zs)` compiles into a single-pass function with zero intermediate allocations.

---

## Phase 22: Real Multi-Result Supercompilation (MRSC) Hypergraph Search

### Context & Problem Statement
`src/mir/supercompiler/mrsc.rs` currently switches between 3 hardcoded compiler passes. Real MRSC (Mitchell & Klyuchnikov 2012) explores a non-deterministic hypergraph of configuration choices.

### Target Files
- `src/mir/supercompiler/mrsc.rs`
- `tests/mrsc_lattice_tests.rs`

### Technical Tasks
1. **Non-Deterministic Configuration Generator**:
   - At each step, enumerate all valid supercompilation actions: unfold, fold against whistle-compatible ancestors, generalize with candidate ancestors, or split terms.
2. **Multi-Result Hypergraph Construction**:
   - Store alternative derivations in an explicit configuration hypergraph.
3. **Pareto Frontier Search**:
   - Implement branch-and-bound graph search to extract the optimal residual program according to user-selected weights over code size, dynamic loop steps, and branch count.

### Verification Gate
- Demonstrate automated discovery of non-trivial optimal residual programs on competing objectives in `tests/mrsc_lattice_tests.rs`.

---

## Phase 23: Real Polyhedral Loop & Stencil Deforestation with Buffer Contraction

### Context & Problem Statement
`src/mir/supercompiler/polyhedral.rs` defined `AffineExpr` without using it, executing only basic forward variable copy substitution. Real polyhedral deforestation requires analyzing affine iteration polyhedra and legally fusing producer-consumer loops to contract intermediate memory arrays.

### Target Files
- `src/mir/supercompiler/polyhedral.rs`
- `tests/polyhedral_stencil_tests.rs`

### Technical Tasks
1. **Iteration Domain & Access Function Extraction**:
   - Extract polyhedral iteration domain inequalities $\{ \vec{i} \mid A \vec{i} + \vec{b} \ge \vec{0} \}$.
   - Extract access matrices $f(\vec{i}) = M \vec{i} + \vec{c}$ for array indexing.
2. **Dependence Vector Analysis**:
   - Compute data dependence distance vectors between producer writes and consumer reads.
3. **Loop Fusion & Buffer Contraction**:
   - When dependence distance is non-negative and bounded, fuse loop nests into a single iteration domain.
   - Contract intermediate array buffers of size $N$ into scalar registers or fixed-size sliding windows.

### Verification Gate
- Multi-pass array stencils (e.g., horizontal blur followed by vertical blur) allocate zero intermediate array buffers in `tests/polyhedral_stencil_tests.rs`.

---

## Phase 24: Formal SMT-Based Translation Validation & Simulation Preorder

### Context & Problem Statement
`src/mir/supercompiler/validate.rs` only executed 256 execution steps, providing shallow testing rather than certified formal translation validation.

### Target Files
- `src/mir/supercompiler/validate.rs`
- `tests/translation_validation_smt_tests.rs`

### Technical Tasks
1. **Verification Condition (VC) Extraction**:
   - Extract relational path formulas between the original MIR CFG and the residualized MIR CFG.
2. **SMT Bit-Vector Encoding**:
   - Encode CFG paths into QF_BV (quantifier-free bit-vectors) and uninterpreted functions.
3. **Simulation Preorder Proof**:
   - Check simulation preorder via an SMT solver (or certified bit-vector decision procedure) proving that for all inputs, residual program outputs match the original program bit-for-bit.

### Verification Gate
- `--verify-equivalence` formally verifies equivalence and detects injected semantic mutations in `tests/translation_validation_smt_tests.rs`.

---

## Phase 25: Genuine Self-Applicable Specializer MinSpec.nl for 2nd and 3rd Futamura

### Context & Problem Statement
The 2nd and 3rd Futamura projections cannot be achieved by running a Rust compiler on NumLang code. Genuine 2nd and 3rd projections require a self-applicable specializer written in the object language itself (`MinSpec.nl`).

### Target Files
- `src/stdlib/minspec.nl`
- `tests/true_futamura_projections_tests.rs`

### Technical Tasks
1. **Self-Applicable Partial Evaluator (`MinSpec.nl`)**:
   - Write a complete partial evaluator in NumLang source code that parses ASTs, tracks static/dynamic environments, and evaluates static expressions.
2. **The 1st Futamura Projection**:
   $$\text{prog\_compiled} = \text{MinSpec}(\text{interp}, \text{prog})$$
3. **The 2nd Futamura Projection (Generating Compiler)**:
   $$\text{compiler} = \text{MinSpec}(\text{MinSpec}, \text{interp})$$
4. **The 3rd Futamura Projection (Compiler-Compiler / Cogen)**:
   $$\text{cogen} = \text{MinSpec}(\text{MinSpec}, \text{MinSpec})$$
5. **Idempotence & Soundness Verification**:
   - Prove that $\text{cogen}(\text{interp}) \equiv \text{compiler}$.
   - Prove that $\text{compiler}(\text{prog}) \equiv \text{prog\_compiled}$.
   - Verify execution equality against the unspecialized interpreter.

### Verification Gate
- Automated test runs all three projections and verifies compiler synthesis and bytecode compilation parity in `tests/true_futamura_projections_tests.rs`.

---

## Phase 26: Rigorous Lean 4 Formal Verification (Zero Axioms, Recursive Semantics)

### Context & Problem Statement
The termination proof in `proof/NumLangProofs/Termination.lean` bypassed the actual proof by smuggling in an unproven `axiom kruskal_tree_theorem`, and `Semantics.lean` modeled only a toy expression language without functions, loops, or recursion.

### Target Files
- `proof/NumLangProofs/Semantics.lean`
- `proof/NumLangProofs/Termination.lean`
- `proof/NumLangProofs/Driving.lean`
- `proof/lakefile.lean`

### Technical Tasks
1. **Extend Language Semantics**:
   - Model mutually recursive function environments $E : \text{String} \to \text{FunctionDef}$, heap pointers, and big-step evaluation $\langle e, \sigma, E \rangle \Downarrow \langle v, \sigma' \rangle$.
2. **Eliminate Unproven Axioms**:
   - Remove `axiom kruskal_tree_theorem`.
   - Construct a genuine mechanized proof of well-quasi-ordering (or finite-alphabet tree embedding) without axioms.
3. **Semantic Preservation Theorem**:
   - Formally prove that driving, folding, and generalization preserve big-step evaluation equivalence.

### Verification Gate
- `lake build` passes with **0 errors, 0 warnings, 0 `sorry`, and 0 `axiom` statements**.

---

## Phase 27: Honest High-Precision Benchmarks & Direct Supercompiler Comparisons

### Context & Problem Statement
`runner.py` measured Windows `CreateProcess` overhead (~14–18 ms) rather than code execution, hid crashes, and contained a double-free bug in C `append3`. Furthermore, zero comparisons existed against real supercompilers from the literature (SPSC, HOSC).

### Target Files
- `bench/harness/runner.py`
- `bench/c/append3.c`
- `bench/benchmarks/`
- `bench/data/results.csv`

### Technical Tasks
1. **In-Process Microsecond Timing**:
   - Wrap core computation in high-iteration loops ($N \ge 10,000$) measuring elapsed execution time inside the binary via hardware performance counters (`QueryPerformanceCounter` / `clock_gettime`).
2. **Fix Baseline Memory Corruptions**:
   - Fix the double-free in `bench/c/append3.c`.
3. **Direct Head-to-Head Comparison against SPSC & HOSC**:
   - Implement and execute canonical supercompiler benchmarks: KMP string matching, Wadler deforestation, Peano multiplication, double reverse, and power specialization.
   - Record compilation time, residual AST node count, and native execution throughput.
4. **Transparent Crash & Latency Reporting**:
   - Enforce `assert returncode == 0`. Record raw measurements directly into `results.csv`.

### Verification Gate
- Automated benchmark execution runs cleanly with 100% exit code 0, populating verified comparison data against SPSC, HOSC, Clang, Rustc, and GHC.

---

## Phase 28: Paper Rewrite & 1-Click Reproducible Artifact Package

### Context & Problem Statement
Table 1 in `paper/main.tex` contained manually altered numbers concealing residualizer crashes, and made unsubstantiated claims regarding 2nd/3rd Futamura projections and formal proofs.

### Target Files
- `paper/main.tex`
- `bench/harness/generate_tables.py`
- `docker/Dockerfile`

### Technical Tasks
1. **Cryptographically Verified Data Pipeline**:
   - Implement `generate_tables.py` to auto-generate `paper/tables/benchmarks.tex` directly from `bench/data/results.csv`, checking SHA-256 hashes to guarantee zero manual tampering.
2. **Full Paper Revision**:
   - Rewrite Section 4 (Futamura Projections) with the verified `MinSpec.nl` implementation.
   - Rewrite Section 5 (Formal Mechanization) with the axiom-free Lean 4 proofs.
   - Rewrite Section 6 (Evaluation) with honest, verified benchmark results and head-to-head SPSC/HOSC data.
3. **Hermetic Docker Reproduction**:
   - Provide a 1-click `make reproduce` command inside Docker that runs all benchmarks, builds the Lean proofs, generates figures, and compiles `paper/main.pdf`.

### Verification Gate
- Single Docker command reproduces all experimental results, figures, and compiles `paper/main.pdf` with complete verification.

---

# Part IV: Advanced Supercompilation, Parallelism & Rigorous Evaluation (Phases 29–40)

---

## Phase 29: Fix Supercompiler Regressions & Boundary Hardening

### Context & Problem Statement
Four benchmark regressions were identified following the initial supercompiler overhaul:
1. `ackermann`: Pathological stack growth and driver timeouts on non-primitive recursive functions.
2. `stream_fusion`: Over-aggressive inlining duplicating loop bodies inside enclosing loops.
3. `fib_matrix`: Order-2 recurrence solver failures panicking rather than falling back to anti-unification.
4. `power_spec`: Degraded performance when supercompilation yielded zero net reductions but altered instruction scheduling.

### Target Files
- `src/mir/supercompiler/drive.rs`
- `src/mir/supercompiler/inliner.rs`
- `src/mir/supercompiler/generalize.rs`
- `src/mir/supercompiler/driver.rs`

### Technical Tasks
1. **Depth-Budgeted Call-Site Knot Tying**:
   - In `drive.rs`, introduce strict call-stack depth limits per call path. When depth budget is reached on functions with non-structural recursion (like Ackermann), force knot formation or fold against existing ancestor configurations.
2. **Loop-Invariant Inliner Call-Site Guards**:
   - In `inliner.rs`, guard against recursive inlining of loops within loops. Only inline leaf functions or functions with bounded call graphs into inner loop blocks.
3. **Anti-Unification (MSG) Fallback for Recurrences**:
   - In `generalize.rs`, ensure that when closed-form linear recurrence solving fails or produces non-integer roots, the driver cleanly falls back to classical Most Specific Generalization (MSG) without panicking.
4. **Unified Profitability Threshold Gate**:
   - In `driver.rs`, measure residual AST complexity and static reduction count. If dynamic operations are not strictly reduced and no loops are deforested, reject the residual MIR and preserve the original CFG.

### Verification Gate
- All 4 previously regressing benchmarks (`ackermann`, `stream_fusion`, `fib_matrix`, `power_spec`) compile without timeout or panic, matching or beating baseline execution time.

---

## Phase 30: Supercompiler Refinements (MSG Knots, Zero-Edge Leaves, Inliner Loop Precomputation, Unified Gate)

### Context & Problem Statement
Edge cases in process-tree construction caused sporadic compiler panics:
1. MSG generalization creating virtual configurations that lacked allocated node IDs for knot targets.
2. Unreachable leaves under budget exhaustion having zero outgoing edges without a terminator.
3. AST inliner repeatedly scanning entire AST trees ($O(N^2)$) to identify whether target functions contained loops.
4. Divergent profitability threshold logic between Classic, Distill, and MRSC modes.

### Target Files
- `src/mir/supercompiler/residualize.rs`
- `src/mir/supercompiler/inliner.rs`
- `src/mir/supercompiler/driver.rs`

### Technical Tasks
1. **Materialize MSG Knot Targets**:
   - In `residualize.rs`, ensure generalized configuration states generated via anti-unification are explicitly materialized in the process tree as distinct addressable nodes before tying back-edge knots.
2. **Handle Zero-Edge Leaves Safely**:
   - In `residualize.rs`, when a non-return process tree leaf has zero outgoing edges due to budget exhaustion or contradiction, emit `Terminator::Unreachable` instead of panicking on missing edges.
3. **Precompute Function Loop Sets**:
   - In `inliner.rs`, precalculate a `HashSet<String>` containing all function names that include loop constructs (`For`, `While`, `Loop`). Query this set in $O(1)$ time during inlining decisions.
4. **Unify Strategy Profitability Gate**:
   - Standardize the metric comparison across Classic, Distillation, and MRSC modes in `driver.rs` using a normalized cost function: $\Delta C = \text{Stmts}_{\text{orig}} - \text{Stmts}_{\text{residual}} + 2 \times \text{DeforestedLoops}$.

### Verification Gate
- Zero panics or crashes across 10,000 randomized fuzz test programs; AST inlining time reduced by $\ge 40\%$ on large files.

---

## Phase 31: Formal Termination Certificate & Order-3 Symbolic Recurrence

### Context & Problem Statement
Academic peer review requires verifiable guarantees that supercompilation terminates on arbitrary NumLang input. Additionally, linear recurrence solving was restricted to order-1 and order-2 recurrences, leaving order-3 recurrences (such as Tribonacci) unsolved.

### Target Files
- `src/mir/supercompiler/whistle.rs`
- `src/mir/supercompiler/generalize.rs`
- `src/codegen/cranelift_backend.rs`
- `src/main.rs`

### Technical Tasks
1. **Machine-Readable Termination Witnesses**:
   - In `whistle.rs`, record each activation of the homeomorphic embedding whistle ($\unlhd$), storing the pair of offending configurations $\langle c_1, c_2 \rangle$, the subterm witness, and the chosen generalization/folding action into a structured `TerminationWitness`.
2. **CLI Termination Certificate Flag**:
   - Add CLI option `--emit-termination-proof <file.json>` in `src/main.rs`, exporting a verifiable JSON certificate containing tree node counts, maximum path depth, whistle firings, and well-quasi-order induction steps.
3. **Order-3 Linear Recurrence Solving**:
   - In `generalize.rs`, implement cubic recurrence solving for relations $s_n = c_1 s_{n-1} + c_2 s_{n-2} + c_3 s_{n-3}$.
   - Add intrinsic `__order3_recurrence(n, c1, c2, c3, s0, s1, s2)` in `cranelift_backend.rs` lowering to unrolled $3 \times 3$ binary matrix exponentiation.

### Verification Gate
- Running `numlang compile --emit-termination-proof cert.json input.nl` outputs a valid JSON schema certificate. Order-3 Tribonacci benchmark executes in $O(\log n)$ time with exact output parity against naive loop.

---

## Phase 32: N-Way Mutual Recurrence Solver (Linear System Solving & Matrix Exponentiation)

### Context & Problem Statement
Scientific computations frequently utilize mutually recursive loops spanning multiple state variables (e.g. coupled differential equation integrators, multi-state Markov chains). Previous solvers could only handle isolated scalar variables.

### Target Files
- `src/mir/supercompiler/generalize.rs`
- `src/mir/supercompiler/driver.rs`
- `src/codegen/cranelift_backend.rs`

### Technical Tasks
1. **Coupled Linear System Extraction**:
   - In `generalize.rs`, implement `NWayLinearSystem` and `detect_nway_linear_system`. Extract transition matrix $M \in \mathbb{Z}^{N \times N}$ ($N \le 8$) and constant vector $B \in \mathbb{Z}^N$ from loop back-edges.
2. **Fraction-Free Gaussian Elimination & Cramer's Rule**:
   - Implement integer linear algebra using Bareiss fraction-free algorithm and Cramer's rule to compute state transitions without floating-point rounding errors.
3. **Binary Matrix Exponentiation**:
   - Implement $O(\log n)$ matrix exponentiation algorithm `mat_pow_nxn` for matrices up to $8 \times 8$.
4. **Native Lowering of N-Way Systems**:
   - Emit `__nway_recurrence_i` intrinsics and unroll matrix multiplications directly in Cranelift backend using SIMD or integer registers.

### Verification Gate
- Coupled 2-way, 3-way, and 4-way mutual recurrences are automatically detected and replaced with $O(\log n)$ matrix exponentiation, verified against unrolled baseline execution.

---

## Phase 33: Refinement Type Propagation Through Process Tree

### Context & Problem Statement
During symbolic driving, branches produce precise interval and equality facts about variables. Failing to propagate these facts resulted in residual code containing impossible branch conditions and redundant array bounds checks.

### Target Files
- `src/mir/supercompiler/state.rs`
- `src/mir/supercompiler/drive.rs`

### Technical Tasks
1. **Interval Arithmetic & Refinement Mapping**:
   - In `state.rs`, attach `refinements: HashMap<Place, Interval>` to `SymbolicState`, where `Interval { min: i64, max: i64 }`.
2. **Branch Condition Narrowing**:
   - In `drive.rs`, when driving conditional branch `br_if v, then_bb, else_bb`:
     - On the `then` edge, intersect variable intervals with the condition (e.g., $x < 10 \implies x.\max = 9$).
     - On the `else` edge, intersect with the negated condition ($x \ge 10 \implies x.\min = 10$).
3. **Dead Branch Pruning**:
   - If an interval intersection yields an empty range ($[\min > \max]$), prune the branch immediately as statically impossible.
4. **Inter-Procedural Argument Refinement**:
   - Pass caller argument interval refinements into inlined callee parameters.
5. **Supercompiler Bounds-Check Elimination**:
   - For array index operations, if the index variable's interval is proven $\subseteq [0, \text{length}-1]$, mark bounds check eliminated (`sc_bce_eliminated`).

### Verification Gate
- Residual code shows 0 bounds-check panics and 0 dead branch paths on bounded loop tests; `sc_bce_eliminated` increments on verified accesses.

---

## Phase 34: Full Higher-Order Closure Driving

### Context & Problem Statement
Functional idioms using `map`, `filter`, and `fold` pass closures across function boundaries. The supercompiler previously treated closures as opaque heap pointers, blocking deforestation of composite pipelines.

### Target Files
- `src/mir/supercompiler/term.rs`
- `src/mir/supercompiler/drive.rs`

### Technical Tasks
1. **First-Class Symbolic Closures**:
   - In `term.rs`, extend `SymTerm` with `ClosureVal(String, Vec<SymTermId>, Type)`, tracking the target function name and symbolic captured values.
2. **Indirect Call Driving**:
   - In `drive.rs`, intercept `Terminator::IndirectCall`. When the callee resolves to a known `ClosureVal`, inline the target function body directly into the process tree.
3. **Environment Unpacking & Argument Binding**:
   - Bind captured environment fields and invocation arguments into the callee's initial symbolic frame.
4. **Higher-Order Deforestation**:
   - Drive chained functional calls (e.g., `xs.map(f).filter(p).fold(0, g)`) into a single loop, eliminating intermediate closure allocations and tuple structures.

### Verification Gate
- Chained `map(filter(xs))` pipeline deforests into a single non-allocating loop in residual MIR, verified via AST inspection and execution tests.

---

## Phase 35: Optimal Residual Code Size (Post-Distillation Compaction)

### Context & Problem Statement
Process-tree supercompilation and distillation frequently create redundant knots, trampoline blocks, and trivial renaming chains, leading to code bloat and cache penalties.

### Target Files
- `src/mir/supercompiler/residualize.rs`
- `src/mir/supercompiler/distill.rs`
- `src/mir/supercompiler/driver.rs`

### Technical Tasks
1. **Pre-Residualization Process-Tree Compaction**:
   - In `compact_process_tree`, prune dead overflow nodes, merge unbranched single-child linear paths, and deduplicate alpha-equivalent knot configurations.
2. **Post-Residualization MIR Peephole Pass**:
   - In `compact_mir_function`, remove identity assignments (`x = x`), eliminate empty basic blocks with unconditional jumps (trampoline folding), and apply conservative copy propagation ($\eta$-reduction).
3. **Residual Statistics Reporting**:
   - Add `residual_block_count` and `residual_stmt_count` to `SupercompilerStats`.
   - Add CLI flag `--supercompile-stats` reporting before-and-after basic block and statement counts.

### Verification Gate
- Code size of residual functions decreases by $\ge 20\%$ across literature benchmarks without changing program semantics.

---

## Phase 36: Parallel Residualization (Independence Detection & Fork/Join Emitting)

### Context & Problem Statement
Supercompiled loops and split process trees execute strictly sequentially on a single thread. When two independent computational knots are discovered, modern multi-core CPUs remain underutilized.

### Target Files
- `src/mir/terminator.rs`
- `src/mir/supercompiler/independence.rs`
- `src/mir/supercompiler/residualize.rs`
- `src/codegen/cranelift_backend.rs`
- `src/codegen/llvm_backend.rs`

### Technical Tasks
1. **MIR Fork/Join Representation**:
   - Extend `Terminator` in `terminator.rs` with `Fork { left: BasicBlockId, right: BasicBlockId, join: BasicBlockId }`.
2. **Read/Write Set Independence Analysis**:
   - In `independence.rs`, implement `ReadWriteSet` analysis for process subtrees. Compute reads and writes to heap locations and global state.
   - Implement `sets_are_independent(s1, s2)` verifying $R(s_1) \cap W(s_2) = \emptyset \land W(s_1) \cap R(s_2) = \emptyset \land W(s_1) \cap W(s_2) = \emptyset$.
3. **Parallel Process-Tree Residualization**:
   - In `residualize.rs`, when independent knot pairs are identified, emit `Terminator::Fork`.
4. **Native Backend Lowering**:
   - Lower `Terminator::Fork` in Cranelift and LLVM to invoke runtime helper `__numlang_fork_join(fn_left, fn_right, arg_left, arg_right)`.
5. **Opt-in CLI Flag**:
   - Expose `--parallel-residualize` CLI flag.

### Verification Gate
- Parallel residualization tests execute with 100% numerical parity; execution on multi-core CPU demonstrates concurrent worker thread execution.

---

## Phase 37: Cross-Module Specialization Cache

### Context & Problem Statement
Supercompilation explores deep symbolic process trees, requiring significant compile time ($O(V^3)$ on complex graphs). Repeatedly supercompiling identical library functions across builds causes unnecessary overhead.

### Target Files
- `src/mir/supercompiler/cache.rs`
- `src/mir/supercompiler/driver.rs`
- `src/main.rs`

### Technical Tasks
1. **Content-Addressed Specialization Cache**:
   - In `cache.rs`, define `CacheKey` combining SHA-256 of the source MIR function, parameter types, caller context, and optimization flags.
2. **Persistent Disk Storage**:
   - Implement 2-level directory fanout (`.numlang_cache/xx/yyyy...json`) storing serialized `CachedSpecialization` (residual MIR and statistics).
3. **Cache Lookup and Store Integration**:
   - In `supercompile_mir_program_with_cache`, perform cache lookup before driving. On hit, deserialize residual MIR and skip process-tree exploration. On miss, drive, residualize, and store into cache.
4. **CLI Cache Controls**:
   - Add CLI options `--cache-dir <dir>` and `--no-cache`.

### Verification Gate
- Secondary compilation with warm cache achieves $\ge 80\%$ cache hit rate and completes $\ge 5\times$ faster than cold compilation.

---

## Phase 38: Full Mechanized Semantic Preservation Proof in Lean 4

### Context & Problem Statement
Academic submission requires mechanized formal verification that supercompilation preserves program semantics. The proof must encompass operational semantics, distillation, MRSC selection, refinement pruning, and compaction.

### Target Files
- `proof/NumLangProofs/Semantics.lean`
- `proof/NumLangProofs/Distillation.lean`
- `proof/NumLangProofs/MRSC.lean`
- `proof/NumLangProofs/Refinement.lean`
- `proof/NumLangProofs/Compaction.lean`
- `proof/NumLangProofs/Main.lean`

### Technical Tasks
1. **Extended MIR Small-Step Semantics**:
   - In `Semantics.lean`, formalize complete operational small-step semantics including environments, heap objects, and function calls.
2. **Hamilton Distillation Soundness**:
   - In `Distillation.lean`, prove that folding across configurations preserves trace equivalence under all valid evaluations.
3. **MRSC Lattice Soundness**:
   - In `MRSC.lean`, prove that selecting any Pareto-optimal configuration from the MRSC hypergraph preserves input-output behavior.
4. **Refinement Branch Pruning Soundness**:
   - In `Refinement.lean`, prove that pruning branches with empty interval bounds does not discard reachable states.
5. **Compaction Soundness & End-to-End Composition**:
   - In `Compaction.lean` and `Main.lean`, compose individual pass soundness lemmas into the master theorem `supercompiler_sound`:
     $$\forall f \in \text{MIR}, \text{Evaluates}(f, \text{env}) = \text{Evaluates}(\text{supercompile}(f), \text{env})$$

### Verification Gate
- `lake build` executes with 0 errors, 0 warnings, 0 `sorry`, and 0 `axiom` statements.

---

## Phase 39: Real-World Benchmark Dominance (30-Benchmark Expansion & Head-to-Head Comparison)

### Context & Problem Statement
The benchmark suite must be broadened from 13 micro-benchmarks to 30 diverse programs to demonstrate consistent performance wins across diverse domains against Clang, GCC, and GHC.

### Target Files
- `bench/benchmarks/`
- `bench/harness/runner.py`
- `bench/harness/generate_tables.py`
- `paper/sections/07_evaluation.tex`

### Technical Tasks
1. **30-Benchmark Suite Expansion**:
   - Add benchmarks across 5 categories:
     - *Pattern Matching*: KMP, Boyer-Moore, Aho-Corasick.
     - *Sorting*: Quicksort, Mergesort, Radix Sort.
     - *Graph Algorithms*: BFS, Dijkstra, PageRank.
     - *Numerical / Scientific*: Matrix Multiplication, FFT, Stencil 2D, N-Body.
     - *Functional Idioms*: Wadler Deforestation, Peano Multiplication, Tree Flip, Ackermann, List Fusion.
2. **Benchmark Taxonomy & Harness**:
   - In `runner.py`, add `BenchmarkEntry` metadata with domains, iteration counts, and baseline command lines.
3. **Statistical Bootstrapping**:
   - Calculate 95% bootstrap confidence intervals across $\ge 30$ runs per benchmark.
4. **Automated LaTeX Table Generation**:
   - In `generate_tables.py`, generate `table_head_to_head.tex` and `table_ablation.tex` directly from raw CSV measurements.
5. **Polyhedral Contraction Safety**:
   - In `src/mir/supercompiler/polyhedral.rs`, verify dependence distances before contracting 2D stencil buffers.

### Verification Gate
- Automated benchmark execution runs across all 30 benchmarks with exit code 0; NumLang supercompilation meets or exceeds Clang -O3 and GHC -O2 on recursive and higher-order benchmarks.

---

## Phase 40: PLDI/ICFP Paper Submission & Artifact Package

### Context & Problem Statement
Finalize the complete academic submission package for PLDI/ICFP, ensuring full reproducibility, modular paper source, and comprehensive responses to potential reviewer criticisms.

### Target Files
- `paper/`
- `docker/Dockerfile`
- `docker/entrypoint.sh`
- `Makefile`
- `bench/data/checksums.sha256`
- `rebuttal/likely_objections.md`
- `tests/supercompiler_phase40_tests.rs`

### Technical Tasks
1. **Modular Paper Structure**:
   - Split `paper/main.tex` into `01_introduction.tex` through `09_conclusion.tex`.
   - Include ACM conference formatting files (`acmart.cls`, `ACM-Reference-Format.bst`).
2. **Complete Bibliography**:
   - Populate `references.bib` with citations for Turchin, Sørensen & Glück, Hamilton, Mitchell & Klyuchnikov, and Bolingbroke & Jones.
3. **Hermetic Docker Artifact**:
   - Provide Dockerfile building Ubuntu container with Rust, Clang, GHC, Lean 4, Python 3, and LaTeX dependencies.
   - Top-level `Makefile` with targets: `make benchmarks`, `make proofs`, `make paper`.
4. **Pre-Emptive Reviewer Rebuttals**:
   - In `rebuttal/likely_objections.md`, address likely reviewer critiques: code size vs speed trade-offs, supercompiler scalability limits, and comparison against modern polyhedral compilers.
5. **Comprehensive Phase 40 Test Suite**:
   - Add `tests/supercompiler_phase40_tests.rs` verifying paper table checksums and artifact build commands.

### Verification Gate
- `docker build` and `make paper` succeed without network access, generating `paper/main.pdf` containing verified benchmark figures and tables.

---

# Part V: Adversarial Remediation & System Soundness (Phases 41–45)

---

## Phase 41: Decouple Win32 & Implement True POSIX Native Codegen

### Context & Problem Statement
NumLang claimed cross-platform native code generation, but Cranelift and LLVM backends unconditionally declared Windows kernel32 symbols (`ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`). Compiling or linking on Linux or inside Docker containers failed with undefined symbol errors during linking (`cc obj.o -o exe -lm`).

### Target Files
- `src/codegen/cranelift_backend.rs`
- `src/codegen/llvm_backend.rs`
- `src/codegen/entry_bench.c`
- `docker/Dockerfile`

### Technical Tasks
1. **Conditional Syscall & Runtime Function Import**:
   - In `cranelift_backend.rs`, conditionally import system functions based on target OS:
     - On Windows: `ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`.
     - On Non-Windows: standard C `exit`, `write`, `malloc` / `mmap`.
2. **Abstract Print and Error Helpers**:
   - Update `emit_helper_print_str` and `emit_bounds_check` to use POSIX file descriptors (`fd = 1` for stdout, `fd = 2` for stderr) via `write(fd, ptr, len)` on non-Windows platforms.
3. **Cross-Platform C Benchmark Entry Point**:
   - In `entry_bench.c`, wrap Windows API calls (`QueryPerformanceCounter`, `ExitProcess`) in `#ifdef _WIN32` blocks, providing POSIX equivalents using `clock_gettime(CLOCK_MONOTONIC)` and standard `exit()`.
4. **Linux & Docker Build Verification**:
   - Add verification tests ensuring compiled object files link cleanly on Linux with `cc obj.o -o exe -lm -no-pie`.

### Verification Gate
- Object files link cleanly on Linux without Windows symbol dependencies; `cargo test` and binary execution succeed in both Windows and Linux/Docker environments.

---

## Phase 42: Replace Vacuous Lean 4 Tautologies with Constructive Proofs

### Context & Problem Statement
An adversarial audit of the formal Lean 4 verification suite revealed that core theorems in `Main.lean`, `Distillation.lean`, and `Compaction.lean` achieved `0 sorry` through circular assumptions: `SemanticEquivalent f1 f2` was asserted as a constructor premise in the theorem hypotheses. Furthermore, `Evaluates` in `Semantics.lean` only checked if return variables existed in the initial environment, entirely bypassing the small-step `Step` relation.

### Target Files
- `proof/NumLangProofs/Semantics.lean`
- `proof/NumLangProofs/Main.lean`
- `proof/NumLangProofs/Distillation.lean`
- `proof/NumLangProofs/Compaction.lean`

### Technical Tasks
1. **Constructive Operational Evaluation**:
   - Redefine `Evaluates fn env res` as the transitive reflexive closure of `Step`:
     $$\text{Evaluates}(fn, env, res) \iff \exists s_f, \text{Step}^* \langle 0, env \rangle s_f \land s_f.term = \text{Return}(res)$$
2. **Formalize Concrete Optimization Passes**:
   - Implement actual transformation functions in Lean (dead code elimination, constant propagation, refinement interval pruning) operating directly on AST/MIR data structures.
3. **Prove Small-Step Simulation Lemmas**:
   - For each transformation pass, prove forward and backward simulation:
     $$\forall s_1 s_2, \text{Step} s_1 s_2 \implies \text{Step}^* (\text{transform}(s_1)) (\text{transform}(s_2))$$
4. **Eliminate Circular Semantic Hypotheses**:
   - Remove `SemanticEquivalent` constructor assumptions from theorem premises. Prove semantic preservation directly by induction on execution traces.

### Verification Gate
- `lake build` verifies all theorems without circular premise assumptions, 0 `sorry`, and 0 `axiom` statements.

---

## Phase 43: Implement Real Self-Applicable Specializer (MinSpec.nl)

### Context & Problem Statement
NumLang claimed execution of the 1st, 2nd, and 3rd Futamura projections in `src/stdlib/minspec.nl`. However, `first_futamura`, `second_futamura_compiler`, and `third_futamura_cogen` had identical function bodies evaluating `min_spec(make_interp_ast(), s_env)`, representing a copy-paste facade rather than genuine self-application.

### Target Files
- `src/stdlib/minspec.nl`
- `tests/futamura_projections_tests.rs`

### Technical Tasks
1. **Complete NumLang Subset Self-Interpreter**:
   - Implement data structures representing NumLang ASTs (`SpecExpr`, `SpecEnv`, `SpecVal`) inside NumLang itself.
2. **Partial Evaluation Engine**:
   - Implement symbolic expression driving and evaluation in `min_spec(spec_prog, static_env)`:
     - If expression subterms are known statically, evaluate them directly.
     - If dynamic, residualize the operation into an output AST.
     - Unfold function calls when recursion is bounded.
3. **Genuine 2nd Futamura Projection (Compiler Generation)**:
   - Implement `second_futamura` as $\text{compiler} = \text{min\_spec}(\text{min\_spec\_ast}, \text{interp\_ast})$.
4. **Genuine 3rd Futamura Projection (Compiler-Generator Generation)**:
   - Implement `third_futamura` as $\text{cogen} = \text{min\_spec}(\text{min\_spec\_ast}, \text{min\_spec\_ast})$.
5. **Idempotence & Equivalence Verification**:
   - Verify that $\text{cogen}(\text{interp\_ast})$ yields a compiler AST that produces identical specialized code.

### Verification Gate
- In `tests/futamura_projections_tests.rs`, assert that the AST produced by `third_futamura` is structurally distinct from the input interpreter and acts as a specialized compiler generator.

---

## Phase 44: Add Memory Management (Scoped Arena or Ref-Counting)

### Context & Problem Statement
NumLang's heap allocation (`Box<T>`, closures) calls `LocalAlloc` or `malloc` with zero deallocation. Long-running numerical loops or recursive heap benchmarks (`nrev`, `tree_flip`) continuously leak memory, preventing deployment in long-running services or embedded devices.

### Target Files
- `src/codegen/cranelift_backend.rs`
- `src/codegen/llvm_backend.rs`
- `src/mir/supercompiler/drive.rs`
- `src/runtime/`

### Technical Tasks
1. **Scoped Arena Allocator Runtime**:
   - Implement an arena allocator in native runtime (`__nl_arena_create`, `__nl_arena_alloc`, `__nl_arena_reset`, `__nl_arena_destroy`).
2. **Loop Iteration Memory Reset**:
   - For supercompiled loops generating intermediate heap nodes (e.g. intermediate cons cells during stream processing), emit `__nl_arena_reset` at the loop back-edge when intermediate data does not escape the iteration.
3. **Automatic Reference Counting (ARC) Pass (Alternative/Complement)**:
   - Implement MIR analysis pass inserting increment instructions on `Box<T>` copies and decrement/deallocate calls when variables go out of scope.
4. **Leak Sanitizer Verification**:
   - Run benchmark suite under AddressSanitizer (`-Zsanitizer=address`) and Valgrind verifying 0 leaked bytes on completion.

### Verification Gate
- Running `tree_flip` and `nrev` for 100,000 iterations maintains flat resident memory usage ($O(1)$ leak rate).

---

## Phase 45: Monolith Decomposition & Codegen Unification

### Context & Problem Statement
`src/codegen/cranelift_backend.rs` has grown into a 9,300+ line monolith combining ABI handling, type translation, instruction emission, intrinsic handling, and Cranelift IR building. Multiple unhandled `.unwrap()` calls risk panics, and legacy AST-level supercompiler passes in `src/opt/` duplicate SSA MIR logic.

### Target Files
- `src/codegen/cranelift/` (`mod.rs`, `abi.rs`, `builder.rs`, `intrinsics.rs`, `emit.rs`)
- `src/opt/` (deprecate legacy passes)
- `src/mir/`

### Technical Tasks
1. **Deconstruct `cranelift_backend.rs` Monolith**:
   - Split `cranelift_backend.rs` into modular subcomponents:
     - `abi.rs`: Target machine configuration, calling conventions, and struct/enum layouts.
     - `intrinsics.rs`: Math intrinsics (`sin`, `cos`, `pow`, `sqrt`), recurrence intrinsics, and syscall helpers.
     - `emit.rs`: Statement and expression lowering to Cranelift CLIF.
     - `mod.rs`: Clean public API `CraneliftCompiler`.
2. **Eliminate Panicking `.unwrap()` Calls**:
   - Audit all `.unwrap()` and `.expect()` calls in backend codegen, converting them to structured `CodegenError` errors with source location context.
3. **Purge Redundant Legacy Passes**:
   - Remove `src/opt/supercompiler/` (the older AST-level prototype), establishing the SSA MIR supercompiler in `src/mir/supercompiler/` as the single authoritative optimization engine.
4. **Shared Code Generation Trait**:
   - Define a unified `BackendCompiler` trait implemented by both Cranelift and LLVM backends.

### Verification Gate
- No source file exceeds 2,500 lines; zero bare `.unwrap()` calls in codegen; `cargo clippy --all-targets -- -D warnings` and all test suites pass with 100% green status.

---

## Phase 46 — Post-Review Loose-Ends Cleanup

**Objective**: Fix all known loose ends surfaced by the Phase 41–45 post-completion codebase review, then re-scan the entire `src/` tree for any additional issues of the same class before closing.

**Rules**:
- Read every target file fully before editing it. Never guess at line numbers or function bodies.
- Do not introduce any `todo!()`, `unimplemented!()`, or new `#[allow(dead_code)]` suppressions.
- After all edits, run `cargo check` and confirm 0 errors, 0 warnings. Then run `cargo test --lib` and confirm all library unit tests pass.
- Do not claim anything is fixed unless `cargo check` output is shown.

---

### Task 1 — Convert 5 `panic!()` calls to `Err(CodegenError::BackendError(...))`

Read the following files in full before editing:
- `src/codegen/cranelift/ast_stmt.rs`
- `src/codegen/cranelift/ast_expr.rs`

Find every `panic!(...)` call in these two files. There are exactly five known sites:
1. `ast_stmt.rs` — "Expected array variable, literal, or array-returning call"
2. `ast_stmt.rs` — "Unsupported array op {}"
3. `ast_stmt.rs` — "Target must be an array variable"
4. `ast_expr.rs` — `unwrap_or_else(|| panic!("Variable '{}' must be found in scope", name))`
5. `ast_expr.rs` — "Index target must be an array variable"

For each site:
- The enclosing function already returns `Result<_, CodegenError>`.
- Replace `panic!(msg)` with `return Err(CodegenError::BackendError(msg.to_string()))`.
- For site 4, convert `unwrap_or_else(|| panic!(...))` into `ok_or_else(|| CodegenError::BackendError(...))?`, matching the pattern used elsewhere in the file.

After editing, grep `src/codegen/cranelift/` for `panic!(` to confirm zero remaining occurrences.

---

### Task 2 — Relocate `try_lower_binary_recurrence_tree` to `src/opt/recursion.rs`

Read in full:
- `src/codegen/cranelift/mod.rs`
- `src/opt/recursion.rs`

The free function `try_lower_binary_recurrence_tree` currently lives in `src/codegen/cranelift/mod.rs`. It is an AST-level transformation (produces a `TypedBlock` from a `TypedFunction`) — the same class as `try_lower_tail_calls` in `src/opt/recursion.rs`.

Steps:
1. Cut `try_lower_binary_recurrence_tree` from `mod.rs` (including its doc comment) and paste it into `src/opt/recursion.rs`, making it `pub(crate)`.
2. In `src/codegen/cranelift/mod.rs`, replace the call `Self::try_lower_binary_recurrence_tree(func)` with `crate::opt::recursion::try_lower_binary_recurrence_tree(func)`.
3. Verify the function's imports (`BinaryOp`, `TypedExpr`, `TypedFunction`, `TypedBlock`, `TypedLiteral`, `TypedStmt`, `Type`) are already available in `recursion.rs`; add any missing ones.
4. Run `cargo check` to confirm.

---

### Task 3 — Remove dead `local_alloc_id` / `os_malloc_id` from `CraneliftCompiler`

Read in full: `src/codegen/cranelift/mod.rs`

The struct `CraneliftCompiler` has two fields suppressed with `#[allow(dead_code)]`:
```rust
#[allow(dead_code)]
pub(crate) local_alloc_id: Option<FuncId>,
#[allow(dead_code)]
pub(crate) os_malloc_id: Option<FuncId>,
```
Remove:
- Both `#[allow(dead_code)]` + field declarations from the struct.
- The platform-conditional `declare_function("LocalAlloc", ...)` / `declare_function("malloc", ...)` blocks in `CraneliftCompiler::new()` that populate them.
- The `local_alloc_id` and `os_malloc_id` entries in the `Ok(Self { ... })` constructor.

After removal, grep for `local_alloc_id` and `os_malloc_id` across `src/` to confirm zero remaining references. Run `cargo check`.

---

### Task 4 — Generalise the `DUMP_CLIF` hardcoded function name

Read in full: `src/codegen/cranelift/mod.rs`

Find the line:
```rust
if std::env::var("DUMP_CLIF").is_ok() && func.name == "solve_nqueens" {
    eprintln!("=== CLIF IR for {} ===\n{}", func.name, ctx.func);
}
```
Replace with:
```rust
if let Ok(dump_target) = std::env::var("DUMP_CLIF") {
    if dump_target.is_empty() || func.name == dump_target {
        eprintln!("=== CLIF IR for {} ===\n{}", func.name, ctx.func);
    }
}
```
Run `cargo check`.

---

### Task 5 — Audit `tests/multi_language_benchmarks.rs` for INTEGRITY_RULES §2.1 compliance

Read the entire file `tests/multi_language_benchmarks.rs`. Look for:
- Any use of `std::process::Command` to time external binaries as a microbenchmark proxy.
- Any hardcoded numeric literals that look like manually entered speedup ratios or expected times.
- Any missing warmup rounds (§2.2 requires ≥5 discarded warmup iterations before measurement).

Report findings exactly as they appear in the file — do not infer or assume. If a §2.1 violation is confirmed (process-spawn timing), fix it to use in-process `std::time::Instant` timing with a warm-up loop. If the file is compliant, state so explicitly with the specific patterns checked.

---

### Task 6 — Re-scan for newly introduced issues of the same class

After Tasks 1–5 are complete, run these commands from the workspace root and show the full output:

```powershell
# 6a: Any new panic!() in codegen paths
Select-String -Path src\codegen\**\*.rs -Pattern 'panic!' -SimpleMatch

# 6b: Any remaining #[allow(dead_code)] suppressions
Select-String -Path src\**\*.rs -Pattern '#\[allow\(dead_code\)\]' -SimpleMatch

# 6c: Any todo!() or unimplemented!()
Select-String -Path src\**\*.rs -Pattern 'todo!|unimplemented!' -SimpleMatch

# 6d: Any bare .unwrap() calls in codegen or MIR
Select-String -Path src\codegen\**\*.rs, src\mir\**\*.rs -Pattern '\.unwrap\(\)' -SimpleMatch
```

For every hit: if it is an intentional use in a test helper, document it and leave it. If it is a real defect, fix it using the same patterns as Tasks 1–5 above.

---

### Verification Gate

- `cargo check` exits 0 with 0 errors and 0 warnings.
- `cargo test --lib` exits 0 with all library unit tests passing.
- `Select-String -Path src\codegen\**\*.rs -Pattern 'panic!'` returns 0 matches.
- `Select-String -Path src\**\*.rs -Pattern '#\[allow\(dead_code\)\]'` returns 0 matches (or every remaining hit is documented as intentional with justification).
- A brief SUMMARY.md is written to `.planning/phases/46-01/SUMMARY.md` listing each task, its outcome, and files changed.

