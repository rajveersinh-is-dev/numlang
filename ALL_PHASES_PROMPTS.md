# NumLang: Complete Master Phase Prompts (Phases 1–28)

> **Document Purpose**: Single authoritative compendium of all execution prompts for the NumLang compiler and supercompiler from **Phase 1 through Phase 28**.  
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
