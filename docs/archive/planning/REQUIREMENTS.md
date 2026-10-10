# Requirements: NumLang Complete Compiler & Supercompiler Requirements

**Defined:** 2026-09-25  
**Updated:** 2026-10-04  
**Governing Standard:** [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md)  
**Core Value:** Mathematically sound, independently reproducible, world-class supercompilation with zero benchmark cheats, zero fake stubs, zero unproven formal axioms, and 100% genuine algorithmic implementations.

---

## Part 0: Foundation Baseline Requirements (Phases 3–9) [COMPLETE]

### 1. Core Control Flow (Phase 3)
- [x] **CF-01**: Lexer tokens: `For`, `In`, `Continue`, `Loop`, `DotDot` (`..`), `DotDotEq` (`..=`).
- [x] **CF-02**: AST nodes and parsing in `src/ast.rs` and `src/parser/stmt.rs` for `Stmt::For`, `Stmt::Continue`, `Stmt::Loop`.
- [x] **CF-03**: Type checker validation for range bounds, loop nesting depth tracking, and rejection of `continue`/`break` outside loop contexts.
- [x] **CF-04**: IR lowering desugaring `Stmt::For` into canonical while loops with induction increments.
- [x] **CF-05**: Cranelift backend support for `continue` jumps targeting active loop header basic blocks.

### 2. Standard I/O Built-ins (Phase 4)
- [x] **IO-01**: Lexer support for `StringLiteral` tokens with escape sequences (`\n`, `\t`, `\\`, `\"`).
- [x] **IO-02**: AST expression nodes for string literals and intrinsic call dispatch for `print` and `println`.
- [x] **IO-03**: Type checker validation allowing primitive types (int, float, bool) and string literals as arguments.
- [x] **IO-04**: Codegen runtime emission using native OS output handles: Win32 `WriteFile` on Windows and POSIX `write(1)` syscalls on Unix.

### 3. Low-Bitwidth Signed Types (Phase 5)
- [x] **TYPE-01**: Lexer recognition for `i8` and `i16` literal suffixes in `TypedIntLiteral`.
- [x] **TYPE-02**: Type system representation `Type::I8` and `Type::I16` with correct size (1, 2 bytes) and alignment.
- [x] **TYPE-03**: Semantic checking for sign-extension, truncating casts, and arithmetic bounds checking.
- [x] **TYPE-04**: Cranelift backend mapping to native `types::I8` and `types::I16` registers.

### 4. Composite Struct Types & Field Access (Phase 6)
- [x] **STRUCT-01**: Lexer tokens `Struct` and `Dot` (`.`).
- [x] **STRUCT-02**: AST definitions and parsing for `struct Name { field: Type }`, struct instantiation `Name { field: val }`, and field access `expr.field`.
- [x] **STRUCT-03**: Type checker symbol table for struct schemas, field offset calculations, and alignment padding rules.
- [x] **STRUCT-04**: Semantic verification ensuring required fields are initialized and field types match declarations.
- [x] **STRUCT-05**: Codegen for stack layout allocation and memory load/store instructions at computed field offsets.

### 5. Pattern Matching (Phase 7)
- [x] **MATCH-01**: Lexer tokens: `Match`, `FatArrow` (`=>`), `Pipe` (`|`), and `Underscore` (`_`).
- [x] **MATCH-02**: AST representation for `Expr::Match` with pattern arms supporting literals, wildcards, and or-patterns.
- [x] **MATCH-03**: Semantic analysis checking scrutinee type against pattern types and enforcing expression type equality across all arm bodies.
- [x] **MATCH-04**: Exhaustiveness checker verifying that boolean matches cover both `true` and `false` or include a wildcard arm.
- [x] **MATCH-05**: Codegen lowering match expressions to multi-way conditional jump ladders or switch tables with phi-merge nodes for result values.

### 6. Production Diagnostic Polish & Explain CLI (Phase 8)
- [x] **DIAG-01**: Catalog standardized compiler error codes `E0001` through `E0025` in `src/diagnostic.rs`.
- [x] **DIAG-02**: Integrate `miette` source snippet rendering with colored underlines, line numbers, and primary/secondary labels.
- [x] **DIAG-03**: Attach exact source `Span`s across lexer tokens, AST nodes, and type-checker errors.
- [x] **DIAG-04**: Implement CLI command `numlang --explain <ERROR_CODE>` printing long-form explanations with problematic and fixed code examples.

### 7. Built-in Standard Library (Phase 9)
- [x] **STD-01**: Mathematical intrinsics: `sqrt`, `abs`, `min`, `max`.
- [x] **STD-02**: Bit manipulation primitives: `popcnt`, `clz`, `ctz`, `bswap`.
- [x] **STD-03**: Floating-point conversions: `i64_to_f64`, `f64_to_i64`.
- [x] **STD-04**: Cranelift intrinsic lowering emitting dedicated machine instructions (e.g. `clz`, `popcnt`, `fsqrt`).
- [x] **STD-05**: Linkage to runtime math functions (`sin`, `cos`, `tan`, `ln`, `exp`) for transcendental operations.

---

## Part I: Core Supercompiler & Language Extensions (Phases 10–19) [COMPLETE]

### 8. Turchin Supercompiler Core, ADTs & 1st Futamura Projection (Phase 10)
- [x] **SC-01**: Symbolic state representation and symbolic driving in `src/mir/supercompiler/drive.rs`.
- [x] **SC-02**: Kruskal homeomorphic embedding whistle in `src/mir/supercompiler/whistle.rs` to detect potential infinite evaluation loops.
- [x] **SC-03**: Generalization and folding mechanisms to tie process tree knots back to ancestor configurations.
- [x] **SC-04**: Closed-form linear recurrence solver detecting accumulator loops and converting $O(N)$ iterations to $O(1)$ arithmetic expressions.
- [x] **SC-05**: 1st Futamura projection: specialize an interpreter against fixed input program AST to eliminate interpreter dispatch.

### 9. Higher-Order Functions, Closures & Pipeline Deforestation (Phase 11)
- [x] **HOF-01**: Lexer and parser support for lambda syntax (`|x, y| expr`) and function types (`fn(T) -> U`).
- [x] **HOF-02**: Type checking of higher-order function arguments, return types, and lexical variable capture.
- [x] **HOF-03**: MIR lowering desugaring closures into environment record structs and function pointer pairs.
- [x] **HOF-04**: Supercompiler driving of indirect calls with known symbolic targets (beta-reduction during driving).
- [x] **HOF-05**: Elimination of intermediate pipeline collections (fusion of `map`, `filter`, and `fold` compositions).

### 10. Polymorphic Types & Generics with Monomorphization (Phase 12)
- [x] **GENERIC-01**: AST syntax for generic parameters on functions (`fn id<T>(x: T) -> T`) and structs (`struct Pair<A, B>`).
- [x] **GENERIC-02**: Representation of type variables (`Type::Param(String)`) in type definitions.
- [x] **GENERIC-03**: Unification-based type inference for generic call sites.
- [x] **GENERIC-04**: Whole-program monomorphization pass in `src/opt/monomorphize.rs` cloning and specializing generic functions for each concrete type combination.
- [x] **GENERIC-05**: Clean Cranelift lowering receiving purely concrete, monomorphic types without boxing overhead.

### 11. Heap Allocation (Box<T>, Deref) & Symbolic Pointer Driving (Phase 13)
- [x] **HEAP-01**: Type system extension `Type::Box(Box<Type>)` representing an owned heap allocation pointer.
- [x] **HEAP-02**: Syntax and parsing for `box expr` allocation and `*expr` dereference expressions.
- [x] **HEAP-03**: MIR statements for heap allocation (`Alloc`), load dereference (`Load`), and store (`Store`).
- [x] **HEAP-04**: Symbolic heap state tracking in `src/mir/supercompiler/state.rs` mapping symbolic locations to term values.
- [x] **HEAP-05**: Driving heap operations symbolically: folding store-load sequences and eliminating intermediate heap allocations.

### 12. Self-Applicable Specializer Prototype & Multistage Futamura (Phase 14)
- [x] **FUTA-PROTO-01**: Implement abstract syntax tree representation and recursive meta-evaluator in NumLang source (`src/stdlib/meta.nl`).
- [x] **FUTA-PROTO-02**: Drive the meta-evaluator symbolically under fixed static program terms.
- [x] **FUTA-PROTO-03**: Verify elimination of interpretation overhead, syntax parsing loops, and dispatch tables.
- [x] **FUTA-PROTO-04**: Formulate multistage specialization harnesses (`tests/third_futamura_tests.rs`).

### 13. Differential Fuzzing (100k Cases) & Lean 4 Setup (Phase 15)
- [x] **FUZZ-L4-01**: Implement random AST generator creating well-typed programs covering arithmetic, branching, loops, and function calls.
- [x] **FUZZ-L4-02**: Implement reference tree-walking interpreter to act as semantic ground truth.
- [x] **FUZZ-L4-03**: Execute 100,000 fuzz programs across both interpreter and supercompiled machine code, comparing outputs.
- [x] **FUZZ-L4-04**: Initialize Lean 4 proof framework (`proof/lakefile.lean`, `Semantics.lean`) with operational semantics scaffolding.

### 14. Canonical Literature Benchmarks & Statistical Harness (Phase 16)
- [x] **BENCH-HARN-01**: Implement canonical workloads: `nrev`, `append3`, `stream_fusion`, `ackermann`, `fib_matrix`, `sieve`, `matvec_4x4`, `raytracer_sphere`, `tree_flip`, `peano_mul`.
- [x] **BENCH-HARN-02**: Provide reference implementations in C (MSVC/Clang), Rust (`rustc -O`), and Haskell (GHC `-O2`).
- [x] **BENCH-HARN-03**: Implement statistical benchmarking harness in `bench/harness/runner.py` with in-process microsecond timing, $\ge 5$ warmup iterations, and $\ge 30$ measurement iterations.
- [x] **BENCH-HARN-04**: Calculate 95% bootstrap confidence intervals, geometric means, and export machine-readable CSV results.

### 15. Multi-Stage Docker Artifact Packaging & PEPM Paper Draft (Phase 17)
- [x] **DOCKER-PEPM-01**: Multi-stage `Dockerfile` packaging Rust, Clang, GHC, Python, and Lean 4 toolchains.
- [x] **DOCKER-PEPM-02**: Reproducibility script `REPRODUCIBILITY.md` and automated evaluation targets.
- [x] **DOCKER-PEPM-03**: Draft academic research paper (`paper/main.tex`) documenting NumLang's architecture and performance.
- [x] **DOCKER-PEPM-04**: Metadata packaging (`.zenodo.json`, `LICENSE`) for artifact submission.

### 16. Global Process-Tree Distillation & MRSC Interface (Phase 18)
- [x] **DIST-MRSC-PROTO-01**: Define global process-tree data structures in `src/mir/supercompiler/distill.rs`.
- [x] **DIST-MRSC-PROTO-02**: Scaffold multi-result configuration hypergraph generation in `src/mir/supercompiler/mrsc.rs`.
- [x] **DIST-MRSC-PROTO-03**: Integrate CLI flags `--mode <classic|distill|mrsc>` and `--mrsc-objective <size|branch|pareto>`.
- [x] **DIST-MRSC-PROTO-04**: Implement unit tests verifying command-line dispatch and baseline transformation passes.

### 17. Polyhedral Stencils, Translation Validation & Parallel Driving (Phase 19)
- [x] **POLY-VAL-01**: Affine iteration domain representation and dependence vector extraction in `src/mir/supercompiler/polyhedral.rs`.
- [x] **POLY-VAL-02**: Symbolic execution translation validator (`src/mir/supercompiler/validate.rs`) checking equivalence between original and residual MIR under `--verify-equivalence`.
- [x] **POLY-VAL-03**: Scoped multi-threaded driving (`src/mir/supercompiler/parallel.rs`) dispatching independent function driving jobs across worker threads (`--threads <N>`).
- [x] **POLY-VAL-04**: Validate that synthetic semantic errors in residual MIR are caught by the translation validator.

---

## Part II: Remediation & Frontier Requirements (Phases 20–28) [COMPLETE]

### 18. Residualization & Generalization (Phase 20)
- [x] **RESID-01**: In `src/mir/supercompiler/residualize.rs`, compute state substitution $\theta$ for every knot edge $N_{\text{curr}} \xrightarrow{\text{Knot}} N_{\text{anc}}$ and emit parallel copy variable assignments or block argument passing.
- [x] **RESID-02**: Remap all `SymTerm::Phi` incoming predecessor basic block IDs to their residual CFG block IDs.
- [x] **RESID-03**: Verify that supercompiled executables for `nrev.nl`, `append3.nl`, `tree_flip.nl`, and `peano_mul.nl` run to completion with exit code `0` and zero crashes.
- [x] **MSG-01**: Implement textbook first-order anti-unification (Sørensen & Glück 1995; Plotkin 1970) in `src/mir/supercompiler/generalize.rs`.
- [x] **MSG-02**: Compute Most-Specific Generalization $\text{msg}(t_1, t_2) = (t_0, \theta_1, \theta_2)$ over symbolic terms when the whistle triggers.
- [x] **MSG-03**: Generalize state environments component-wise and resume driving with fresh generalization variables.

### 19. Hamilton Global Distillation (Phase 21)
- [x] **DISTILL-01**: Implement a global process tree representation in `src/mir/supercompiler/distill.rs` modeling call configurations across the entire call graph.
- [x] **DISTILL-02**: Implement a global whistle and inter-procedural folding mechanism across distinct function definitions.
- [x] **DISTILL-03**: Verify automated deforestation of composed recursive functions (e.g., `append (append xs ys) zs` $\to$ single-pass 3-argument function without intermediate list allocations).

### 20. Multi-Result Supercompilation (Phase 22)
- [x] **MRSC-01**: Implement a non-deterministic configuration hypergraph generator in `src/mir/supercompiler/mrsc.rs` branching on driving, folding, and generalization choices.
- [x] **MRSC-02**: Build a configuration lattice search exploring the space of valid residual programs.
- [x] **MRSC-03**: Implement Pareto-optimal residualization search extracting optimal programs according to user-selected metrics (code size, step count, branch count).

### 21. Polyhedral Loop & Stencil Deforestation (Phase 23)
- [x] **POLY-01**: Extract affine iteration domain polyhedra $\{ \vec{i} \mid A \vec{i} + \vec{b} \ge \vec{0} \}$ and access matrices in `src/mir/supercompiler/polyhedral.rs`.
- [x] **POLY-02**: Compute data dependence distance vectors between producer loops and consumer loops.
- [x] **POLY-03**: Perform legal affine loop fusion and contract intermediate array buffers to $O(1)$ scalar temporaries or sliding windows.

### 22. Formal SMT-Based Translation Validation (Phase 24)
- [x] **VALID-01**: Extract Verification Conditions (VCs) and relational path formulas between original and residual MIR CFGs in `src/mir/supercompiler/validate.rs`.
- [x] **VALID-02**: Encode paths and invariant assertions into QF_BV (quantifier-free bit-vectors) SMT formulas.
- [x] **VALID-03**: Formally prove simulation preorder over all execution paths under `--verify-equivalence`.

### 23. Genuine Futamura Projections (Phase 25)
- [x] **FUTA-01**: Implement a self-contained, self-applicable partial evaluator `MinSpec.nl` in NumLang source code (`src/stdlib/minspec.nl`).
- [x] **FUTA-02**: Verify 1st Futamura projection: $\text{MinSpec}(\text{interp}, \text{prog}) \to \text{prog\_compiled}$.
- [x] **FUTA-03**: Verify 2nd Futamura projection: $\text{MinSpec}(\text{MinSpec}, \text{interp}) \to \text{compiler}$.
- [x] **FUTA-04**: Verify 3rd Futamura projection: $\text{MinSpec}(\text{MinSpec}, \text{MinSpec}) \to \text{cogen}$ and prove $\text{cogen}(\text{interp}) \equiv \text{compiler}$.

### 24. Rigorous Lean 4 Formal Verification (Phase 26)
- [x] **LEAN-01**: Remove `axiom kruskal_tree_theorem` from `proof/NumLangProofs/Termination.lean` and prove termination constructively without axioms.
- [x] **LEAN-02**: Extend `proof/NumLangProofs/Semantics.lean` to model recursive function environments, heap memory, and control flow.
- [x] **LEAN-03**: Mechanize the soundness theorem proving that driving, folding, and generalization preserve big-step operational semantics, compiling cleanly with 0 `sorry` and 0 `axiom`s.

### 25. Honest High-Precision Benchmarks (Phase 27)
- [x] **BENCH-01**: Rewrite `bench/harness/runner.py` to use in-process microsecond hardware performance counter timing across $\ge 10,000$ iterations.
- [x] **BENCH-02**: Validate process exit codes (`assert returncode == 0`) and report any crashes explicitly as `ERROR`.
- [x] **BENCH-03**: Fix memory bugs in C baselines (fix `append3` double-free).
- [x] **BENCH-04**: Benchmark NumLang head-to-head against SPSC and HOSC on canonical literature benchmarks (KMP, Wadler deforestation, Peano multiplication, etc.).

### 26. Paper Rewrite & Artifact Evaluation (Phase 28)
- [x] **PAPER-01**: Rewrite `paper/main.tex` with automated SHA-256 data pipeline directly populating tables from `bench/data/results.csv`.
- [x] **PAPER-02**: Accurately describe verified algorithms, honest limitations, and measured speedups without data fabrication.
- [x] **PAPER-03**: Package a hermetic multi-stage Docker container where `make reproduce` compiles `paper/main.pdf` in one command.

---

## Part III: Adversarial Remediation Requirements (Phases 41–47) [COMPLETE]

### 27. Win32 Decoupling & POSIX Codegen (Phase 41)
- [x] **PORT-01**: In `src/codegen/cranelift_backend.rs`, conditionally declare Win32 APIs only when targeting Windows. On non-Windows platforms, declare standard C library `exit` and `write`.
- [x] **PORT-02**: In `src/codegen/llvm_backend.rs`, dynamically target the host triple and emit conditional `exit` and `write` signatures for non-Windows targets.
- [x] **PORT-03**: Update `src/codegen/entry_bench.c` with `#ifdef _WIN32` portability branch using `clock_gettime(CLOCK_MONOTONIC)` on POSIX.
- [x] **PORT-04**: Implement checked integer arithmetic (`checked_mul`, `checked_add`) and non-zero divisor guard in `src/mir/supercompiler/generalize.rs` for order-2 linear recurrences.
- [x] **PORT-05**: Create `tests/platform_portability_tests.rs` verifying absence of undefined Windows symbols on non-Windows builds.

### 28. Constructive Lean 4 Operational Proofs (Phase 42)
- [x] **LEAN-04**: In `lean/Supercompiler/Semantics.lean`, define small-step `Step` relation and transitive closure `StepStar`. Define `TerminatesWith` and re-anchor `Evaluates`.
- [x] **LEAN-05**: Prove operational preservation lemmas (stepstar_trans, stepstar_single, const_fold_stmt_equiv, drive_step_preserves_semantics).
- [x] **LEAN-06**: Eliminate circular hypotheses from `Preservation.lean` (`DriveStep`), `Compaction.lean` (`NoopRemoval`, `EtaReduction`), and `Distillation.lean` (`FoldStep`).
- [x] **LEAN-07**: Prove end-to-end `supercompiler_sound` in `Main.lean`.
- [x] **LEAN-08**: Provide test suite `tests/constructive_lean4_phase42_tests.rs` verifying 0 `sorry`, 0 `axiom`, and successful `lake build`.

### 29. Full Futamura Projections (Phase 43)
- [x] **FUTA-05**: Expand `src/stdlib/minspec.nl` with full recursive AST representations (`SpecExpr` with literals, variables, binary operators, if-conditions, function calls).
- [x] **FUTA-06**: Implement 1st Futamura projection: $\text{Specialize}(\text{interp}, \text{prog}) \to \text{target\_prog}$.
- [x] **FUTA-07**: Implement 2nd Futamura projection: $\text{Specialize}(\text{specialize}, \text{interp}) \to \text{compiler}$.
- [x] **FUTA-08**: Implement 3rd Futamura projection: $\text{Specialize}(\text{specialize}, \text{specialize}) \to \text{cogen}$.
- [x] **FUTA-09**: Verify structural divergence $\text{AST}(\text{cogen}) \neq \text{AST}(\text{compiler}) \neq \text{AST}(\text{interp})$ and 0 copy-paste duplication.

### 30. Scoped Arena & Memory Safety (Phase 44)
- [x] **FUZZ-01**: Implement arena allocator runtime in `src/runtime/arena.rs` and `src/runtime/arena.c` with chunk management, peak tracking, and reset functions (`__nl_arena_create`, `__nl_arena_alloc`, `__nl_arena_reset`, `__nl_loop_reset`).
- [x] **FUZZ-02**: Integrate arena fast-path into `src/codegen/cranelift/` and `src/codegen/cranelift_backend.rs` (`__nl_arena_cur`, `__nl_arena_end`).
- [x] **FUZZ-03**: Implement loop escape analysis (`should_reset_loop_iteration`) in backend and `residualize.rs` to emit `__nl_loop_reset()` latch at loop back-edges for non-escaping allocations.
- [x] **FUZZ-04**: Verify 50,000-iteration memory bounds and zero memory corruption in `tests/memory_leak_tests.rs`.

### 31. Monolith Decomposition & SMT Loop Validation (Phase 45)
- [x] **CODEGEN-01**: Partition Cranelift backend into `src/codegen/cranelift/` (`mod.rs`, `abi.rs`, `intrinsics.rs`, `escape.rs`, `ast_stmt.rs`, `ast_expr.rs`, `mir_emit.rs`). Retain `src/codegen/cranelift_backend.rs` as a thin forwarding shim.
- [x] **CODEGEN-02**: Enforce strict file size bound: no file exceeds 2,500 lines.
- [x] **CODEGEN-03**: Define `BackendCompiler` trait in `src/codegen/backend_trait.rs` and implement for `CraneliftCompiler` and `LlvmCompiler`.
- [x] **CODEGEN-04**: Replace all bare `.unwrap()` / `.expect()` in codegen with structured `CodegenError` variants.
- [x] **CODEGEN-05**: Purge deprecated `src/opt/supercompiler/` directory.
- [x] **CODEGEN-06**: Implement $k$-induction loop translation validation in `src/mir/supercompiler/validate.rs`.

### 32. Post-Review Loose-Ends Cleanup (Phase 46)
- [x] **CLEAN-01**: In `src/codegen/cranelift/ast_stmt.rs` and `ast_expr.rs`, convert all remaining 5 `panic!()` sites to structured `Err(CodegenError::BackendError(...))`.
- [x] **CLEAN-02**: Relocate `try_lower_binary_recurrence_tree` from `src/codegen/cranelift/mod.rs` to `src/opt/recursion.rs` alongside tail-call optimization.
- [x] **CLEAN-03**: Remove dead `local_alloc_id` and `os_malloc_id` fields and unused system allocator declarations from `CraneliftCompiler`.
- [x] **CLEAN-04**: Generalize `DUMP_CLIF` in `src/codegen/cranelift/mod.rs` to dump all functions when empty or target an arbitrary function name string.
- [x] **CLEAN-05**: Harden `tests/multi_language_benchmarks.rs` to enforce $\ge 5$ discarded warmup iterations before timing measurement.

### 33. Hardening, Clippy Purity & Safety Audit (Phase 47)
- [x] **HARDEN-01**: Eliminate all `clippy::needless_return` instances in `src/codegen/cranelift/ast_expr.rs` and `ast_stmt.rs`.
- [x] **HARDEN-02**: Update `tests/platform_portability_tests.rs` to inspect both `mod.rs` and `intrinsics.rs` with `Linkage::Import`.
- [x] **HARDEN-03**: Eliminate 20+ repetitive `.unwrap()` calls in `src/mir/lower.rs` with safe monadic helpers `push_stmt`, `current_block_id`, `set_terminator`, and `current_terminator`.
- [x] **HARDEN-04**: Extend `CodegenError` in `src/codegen/cranelift/abi.rs` with domain-specific variants (`VariableNotFound`, `InvalidArrayTarget`, `MissingLayout`, `FieldNotFound`, `UnsupportedOp`).
- [x] **HARDEN-05**: Ensure whole-workspace `cargo clippy --all-targets -- -D warnings` and `cargo check --tests` pass with zero warnings.

---

## Part IV: Global Dominance & Algorithmic Generality Requirements (Phases 48–50) [COMPLETE]

### 34. Total Algorithmic Generality & Structural Name Decoupling (Phase 48)
- [x] **GEN-01**: Implement `detect_symmetric_permutation_recurrence` in `src/opt/recursion.rs` to detect 3-way cyclic argument permutations with decrements ($\pi_1=(x-1,y,z), \pi_2=(y-1,z,x), \pi_3=(z-1,x,y)$) and branch conditions ($x \le y$), contracting Takeuchi recurrences by structural induction regardless of function/variable names.
- [x] **GEN-02**: Implement bounded symbolic induction for nested deep recurrences in `src/opt/recursion.rs`, specializing affine parameter slices ($m \in \{1, 2\}$) via symbolic driving and arithmetic progression detection to replace hardcoded Ackermann identities.
- [x] **GEN-03**: Replace `func.name == "append3"` in `src/mir/supercompiler/mod.rs` and `lower.rs` with structural check `is_nested_recursive_composition(func)` and MIR `is_distilled` flag.
- [x] **GEN-04**: Replace hardcoded field name heuristics (`"x"`, `"y"`, `"z"`, `"first"`) in `src/codegen/llvm_backend.rs` with a type-directed `struct_fields: HashMap<String, Vec<(String, Type)>>` populated directly from `program.structs`.
- [x] **GEN-05**: Verify structural generality in `tests/structural_generality_tests.rs` using renamed/obfuscated symbols across all literature recurrence benchmarks.

### 35. Deep Reynolds Defunctionalization & Higher-Order Deforestation (Phase 49)
- [x] **DEFUN-01**: Implement whole-program type-directed Reynolds defunctionalization in `src/mir/defunctionalize.rs`, generating discriminated union sum types `ClosureTag_<Sig>` for each closure call signature.
- [x] **DEFUN-02**: Replace `Rvalue::ClosureAlloc` with typed tagged enum allocations carrying captured environment payloads.
- [x] **DEFUN-03**: Lower `Terminator::IndirectCall` into direct `Terminator::SwitchInt` over tags, dispatching to monomorphic static `Terminator::Call` sites.
- [x] **DEFUN-04**: Drive through closure tags in `src/mir/supercompiler/drive.rs` and deforest higher-order pipelines (`map`/`filter`/`fold`) in `src/mir/supercompiler/distill.rs` into single-pass, allocation-free loops.
- [x] **DEFUN-05**: Eliminate intermediate closure objects via SROA and verify zero allocations and zero indirect calls in `tests/defunctionalize_deforestation_tests.rs`.

### 36. Supercompiler-to-LLVM Co-Optimization Engine (Phase 50)
- [x] **COOPT-01**: Emit Type-Based Alias Analysis (`!tbaa`) trees and `noalias` attributes in `src/codegen/llvm_backend.rs` proving disjointness of struct fields and distinct heap slices.
- [x] **COOPT-02**: Emit `llvm.loop.vectorize.enable` and `llvm.loop.unroll.enable` metadata on deforested/distilled loops proven dependency-free by polyhedral analysis.
- [x] **COOPT-03**: Lower order-$N$ recurrence matrix powers to unrolled $2\times 2$ and $4\times 4$ SIMD vector operations (`llvm.x86.avx2` / auto-vectorized f64x4).
- [x] **COOPT-04**: Implement production-grade two-level content-addressed SHA-256 specialization disk cache in `src/mir/supercompiler/cache.rs`.
- [x] **COOPT-05**: Implement canonical dominance benchmark suite in `tests/supercompiler_llvm_dominance_tests.rs` proving NumLang outperforms `clang -O3`, `gcc -O3`, `rustc -O3`, and `ghc -O3`.

---

## Part V: Total Unconditional Dominance Requirements (Phases 51–56) [COMPLETE]

### 37. Lazy/Thunk SSA Extension & Codata Supercompilation (Phase 51)
- [x] **LAZY-01**: Define `Rvalue::Thunk { body: MirBodyId, env: Vec<LocalId> }` and `Terminator::Force { thunk: LocalId, result: LocalId, cont: BasicBlockId }` in `src/mir/mod.rs`. Extend `MirPrinter` and validation pass.
- [x] **LAZY-02**: Implement `src/mir/thunk_analysis.rs`: demand-propagation analysis computing which thunks are demanded on every execution path, enabling selective forcing at compile-time during driving.
- [x] **LAZY-03**: Implement **lazy driving mode** in `src/mir/supercompiler/drive.rs`: when driving a `Force` terminator, symbolically evaluate the thunk body only when the result is used. Represent unevaluated thunks as `SymTerm::Thunk(MirBodyId, Vec<SymTermId>)` in the symbolic state.
- [x] **LAZY-04**: Implement **stream fusion** in `src/mir/supercompiler/distill.rs`: recognize producer-consumer thunk chains (`map`, `filter`, `take`, `zipWith`, `iterate`) and fuse them into a single allocation-free loop without intermediate stream nodes.
- [x] **LAZY-05**: Verify in `tests/codata_supercompilation_tests.rs` that: lazy streams fuse without divergence, `take N (zipWith f xs ys)` produces zero intermediate allocations, and resulting LLVM IR contains no heap allocation calls.

### 38. Speculative Type Guards & Deoptimization Safepoints (Phase 52)
- [x] **DEOPT-01**: Implement `src/mir/speculate.rs`: type-profile analysis that records the set of concrete types observed at each polymorphic call site in the process tree symbolic state.
- [x] **DEOPT-02**: Emit **fast-path type guards** in residualized MIR: at each polymorphic call site, emit `Terminator::TypeGuard { local, expected_tag, fast_path, deopt_stub }` which branches to a monomorphic fast path when the tag matches, and a deoptimization stub otherwise.
- [x] **DEOPT-03**: Implement **deoptimization stubs** in `src/codegen/cranelift/deopt.rs`: stubs that reconstruct the unspecialized interpreter call frame from the current register state and resume execution at the unspecialized function entry.
- [x] **DEOPT-04**: Implement **on-stack replacement (OSR) entry points** in Cranelift codegen: function preambles with OSR transition slots allowing hot-path upgrade from unspecialized to specialized code at function entry boundaries.
- [x] **DEOPT-05**: Verify in `tests/speculative_deopt_tests.rs` that: polymorphic `i64`/`f64` call sites run the specialized fast path 99%+ of calls with zero deopt on uniform-type input, and correctly deoptimize and produce correct output on type-mismatch input.

### 39. Exhaustive MRSC Oracle with IDDFS & Pareto Cost Model (Phase 53)
- [x] **ORACLE-01**: Implement `src/mir/supercompiler/mrsc_oracle.rs`: an iterative deepening depth-first search (IDDFS) over the MRSC configuration hypergraph with configurable depth bound, invoked by `--mrsc-exhaustive` CLI flag.
- [x] **ORACLE-02**: Implement `MrscCostModel` in `src/mir/supercompiler/mrsc.rs`: a cost oracle computing (a) dynamic step count via symbolic unrolling, (b) heap allocation count, (c) residual basic block count, and (d) live register pressure for each candidate residual program.
- [x] **ORACLE-03**: Implement **Pareto frontier extraction**: given the set of all residual programs produced by IDDFS, extract the Pareto-dominant subset under the 4-dimensional cost model and select the program minimizing a user-configurable linear combination (e.g. `--mrsc-objective speed`).
- [x] **ORACLE-04**: Integrate the winning IDDFS residual into the L2 disk specialization cache (`src/mir/supercompiler/cache.rs`) keyed by the function's structural hash, bypassing future online driving for that configuration.
- [x] **ORACLE-05**: Verify in `tests/mrsc_oracle_tests.rs` that IDDFS with depth $\ge 20$ discovers residuals strictly smaller (by step count) than the bounded online MRSC, and that Pareto selection is reproducible and deterministic across runs.

### 40. Pre-Defunctionalization Higher-Order AST Distillation (Phase 54)
- [x] **HODIST-01**: Implement `src/ast/hodistill.rs`: a **lambda-level global process-tree distillation pass** operating directly on the typed functional AST before MIR lowering. Model lambda terms as process tree nodes with `Call`, `Lam`, `App`, `Let`, `Case` constructors.
- [x] **HODIST-02**: Implement **inter-procedural folding** in `hodistill.rs`: when the process tree revisits a configuration alpha-equivalent to an ancestor configuration modulo variable renaming, fold back to the ancestor with a generalized accumulator parameter (Hamilton's distillation fold rule).
- [x] **HODIST-03**: Implement **higher-order deforestation** in `hodistill.rs`: recognize chains of composed higher-order functions (`compose f (compose g h) x`) and produce a single fused function application via the distillation unfolding rule, eliminating all intermediate lambda closures before they reach MIR.
- [x] **HODIST-04**: Wire `hodistill::distill_program` into the compilation pipeline in `src/compiler.rs`, running *before* `lower_to_mir`, and controlled by `--ho-distill` CLI flag (enabled by default in `--supercompile` mode).
- [x] **HODIST-05**: Verify in `tests/ho_ast_distillation_tests.rs` that: 5-deep `compose` chains fuse into single-pass applications, mutual recursion across 3 higher-order functions distills into a single loop, and the distilled AST contains zero intermediate closure-returning function applications.

### 41. Pure-Rust Polyhedral ILP Scheduler (Pluto-style) (Phase 55)
- [x] **ILP-01**: Implement `src/mir/supercompiler/polyhedral_ilp.rs`: a **fraction-free Simplex method** over integer polyhedra (Bareiss algorithm for exact integer pivoting) to solve the Farkas lemma dual LP for loop schedule feasibility.
- [x] **ILP-02**: Implement **Pluto-style permutability conditions**: for each pair of loop statements `S_i`, `S_j` with dependence distance vector `d`, emit the scheduling constraint $\theta_i \cdot d \ge 0$ as a linear inequality and feed into the Simplex solver to compute a legal permutable schedule.
- [x] **ILP-03**: Implement **loop tiling** in `src/mir/supercompiler/polyhedral.rs`: given a permutable schedule from the ILP, apply rectangular tiling with tile sizes derived from cache capacity estimates (default 32-element tiles for i64 arrays targeting L1 cache).
- [x] **ILP-04**: Emit tiled and vectorized loop nests in residualized MIR: the innermost tile dimension maps to `Terminator::Fork` parallel blocks or LLVM vectorization metadata.
- [x] **ILP-05**: Verify in `tests/polyhedral_ilp_tests.rs` that: a 3-nested matrix multiply loop transforms to a tiled schedule legal under all dependences, intermediate buffers contract to scalar temporaries, and the Simplex solver terminates in $\le 1,000$ pivot steps for loops with $\le 8$ dimensions.

### 42. Post-Residualization Outlining & Tiered JIT Compilation (Phase 56)
- [x] **OUTLINE-01**: Implement `src/mir/supercompiler/outliner.rs`: content-addressed hashing of normalized basic block instruction sequences (opcodes + operand types, modulo register names). Detect duplicate sequences across specialization variants with similarity threshold $\ge 90\%$.
- [x] **OUTLINE-02**: Extract duplicated sequences into shared outlined subroutines with explicit parameter lists derived from the live variable set at the extracted boundary. Replace all original sites with `Terminator::Call` to the shared subroutine.
- [x] **OUTLINE-03**: Implement **tiered compilation** in `src/compiler.rs`: Tier 0 — direct MIR $\to$ Cranelift with zero supercompilation (< 2ms cold start); Tier 1 — full supercompilation triggered asynchronously on functions whose call count exceeds a configurable hot threshold (default 100 calls).
- [x] **OUTLINE-04**: Implement **background supercompilation thread** in `src/runtime/tier.rs`: a `SupercompileWorker` thread that receives `(FunctionId, SpecializationKey)` messages, runs the full supercompiler pipeline, and atomically swaps the Cranelift function pointer via a `std::sync::atomic::AtomicPtr`.
- [x] **OUTLINE-05**: Verify in `tests/tiered_jit_tests.rs` that: Tier 0 cold startup for a 10-function program completes in < 5ms, Tier 1 upgrade fires after the hot threshold is crossed, and binary size under outlining is $\le 120\%$ of a non-specialized baseline (vs $\le 300\%$ without outlining).

---

## Part VI: World's Fastest General Supercompiler Requirements (Phases 57–66)

### 43. Rigorous Differential Validation & Lean Operational Equivalence (Phase 57) [COMPLETE]
- [x] **DIFF-01**: Implement `src/testing/gen.rs`: a `proptest` strategy generating well-typed NumLang programs covering arithmetic, recursion, conditionals, `Box<T>`, and closures, with guaranteed structural termination.
- [x] **DIFF-02**: Implement `src/testing/oracle.rs`: an independent pure-Rust tree-walking AST interpreter serving as ground truth oracle (up to 10M step budget).
- [x] **DIFF-03**: Create `tests/differential_validation_tests.rs`: run generated programs through all 5 execution paths and assert output equivalence.
- [x] **DIFF-04**: Bridge random AST generation to Lean 4 formal semantics: serialize generated ASTs into Lean definitions and run `lake exe step_checker` to verify operational semantics agreement.
- [x] **DIFF-05**: Run 10,000 generated programs with zero discrepancies across all backends.

### 44. CLBG Loss Diagnosis & Fix Plan (Phase 58) [COMPLETE]
- [x] **CLBG-01**: Early cutoff for branching loop bodies: when `try_solve_accumulator_loop` fails due to conditional bodies (e.g. `pidigits` alternating sum), set forced knot cutoff to `header_visits >= 1` instead of unrolling 12 iterations.
- [x] **CLBG-02**: Small constant loop unrolling guard: when loop bound is constant but body contains non-pure calls (`binary_trees`), prevent partial unrolling bloat.
- [x] **CLBG-03**: MSG generalization for knot transfers: generalize differing variable states so knots can tie naturally before forced cutoffs.
- [x] **CLBG-04**: Code size bloat guard: if residual basic block count exceeds $3\times$ source block count without closed-form collapse, reject specialization and preserve baseline MIR.
- [x] **CLBG-05**: Verify CLBG suite performance (`spectral_norm`, `nbody`, `fannkuch_redux`, `mandelbrot`, `pidigits`, `binary_trees`) beats or matches baseline C/MSVC /O2.

### 45. Algebraic Identity Reduction in Term Interning (Phase 59) [COMPLETE]
- [x] **ALG-01**: Canonical commutative ordering in `intern_binary`: enforce strict operand ordering for commutative operators (`+`, `*`, `^`, `&`, `|`, `==`, `!=`) ensuring symmetric terms map to identical IDs.
- [x] **ALG-02**: Core algebraic simplification rules: zero, one, identity, and annihilation laws (`x + 0 = x`, `x * 1 = x`, `x * 0 = 0`, `x - x = 0`, `x / x = 1`, `x & 0 = 0`, `x | 0 = x`, `x ^ x = 0`).
- [x] **ALG-03**: Unary simplification: double negation reduction (`¬¬x = x`, `--x = x`) and comparison inversion under negation (`!(a < b) = a >= b`).
- [x] **ALG-04**: Floating-point constant folding and identity laws: constant evaluation for all float arithmetic and intrinsic evaluations (`sqrt`, `abs`) using standard IEEE-754 semantics.
- [x] **ALG-05**: Verification in `tests/algebraic_reduction_tests.rs`: verify 40+ algebraic identities across integer and floating-point types; ensure 100% pass rate under differential validation.

### 46. Nonlinear Polynomial Recurrence Solver (Phase 60) [COMPLETE]
- [x] **POLYREC-01**: Implement quadratic recurrence detection and closed-form polynomial sum formulas: $\sum_{k=1}^n k = \frac{n(n+1)}{2}$, $\sum_{k=1}^n k^2 = \frac{n(n+1)(2n+1)}{6}$, $\sum_{k=1}^n k^3 = \left(\frac{n(n+1)}{2}\right)^2$ in `src/mir/supercompiler/recurrence.rs`.
- [x] **POLYREC-02**: Implement geometric series solver: recognize $acc_{k} = acc_{k-1} + c \cdot r^k$ with invariant ratio $r \neq 1$, generating closed form $c \cdot \frac{r^{n+1} - 1}{r - 1} + init$ using symbolic binary exponentiation (`sym_pow`).
- [x] **POLYREC-03**: Implement exponential power recurrence solver: recognize $acc_{k} = acc_{k-1} \cdot k$ where $k$ is loop-invariant, generating $init \cdot k^n$ in $O(\log n)$ operations.
- [x] **POLYREC-04**: Integrate polynomial/geometric solver into `try_solve_loop_recurrence` in `src/mir/supercompiler/drive.rs` so that when linear matrix analysis fails, the nonlinear polynomial and geometric detectors fire automatically.
- [x] **POLYREC-05**: Verification in `tests/polynomial_recurrence_tests.rs`: verify closed-form collapse and exact numerical equivalence for square pyramid sums, geometric sums, and exponential power loops.

### 47. Fast Hash-Cons Whistle: O(1) Structural Identity (Phase 61) [COMPLETE]
- [x] **HASHCONS-01**: Enforce that structurally identical terms have the same `SymTermId` as an invariant of `TermInterner`.
- [x] **HASHCONS-02**: Add a DAG depth cache (`depths: Vec<usize>`) and precomputed node size cache (`sizes: Vec<usize>`) to `TermInterner` so that embedding size filters operate in $O(1)$ time.
- [x] **HASHCONS-03**: Compute structural FxHash for every `SymTermId` slot (`hashes: Vec<u64>`), allowing $O(1)$ equality tests before full structural tree comparison.
- [x] **HASHCONS-04**: Optimize `is_embedded` and `state_embeds` in `src/mir/supercompiler/whistle.rs` using size and depth lower-bounds to skip non-embedding candidates.
- [x] **HASHCONS-05**: Verification in `tests/fast_whistle_tests.rs`: benchmark whistle performance on programs with 50+ symbolic variables; achieve $\ge 5\times$ throughput speedup on large synthetic terms.

### 48. Whole-Program Cross-Function Recurrence Closing (Phase 62) [COMPLETE]
- [x] **XFUNC-01**: In `drive.rs`, detect inter-procedural call cycles in the process tree where function $A$ calls $B$ which calls $A$.
- [x] **XFUNC-02**: Extract joint transition matrices representing the composite state transformation across the mutual call cycle.
- [x] **XFUNC-03**: Interface with `solve_nway_recurrence` in `src/mir/supercompiler/recurrence.rs` to construct an $N \times N$ companion matrix for the coupled system.
- [x] **XFUNC-04**: Emit residualized matrix exponentiation or closed-form expressions that compute the mutual recursion result in $O(\log N)$ or $O(1)$.
- [x] **XFUNC-05**: Verification in `tests/mutual_recursion_collapse_tests.rs`: verify closed-form collapse of `even/odd`, 2-level mutual recursions, and Hofstadter-style linear systems.

### 49. True Production Self-Applicable Specializer (2nd Futamura Binary Output) (Phase 63) [COMPLETE]
- [x] **PROD-FUTA2-01**: Upgrade `MinSpec.nl` to accept serialized AST byte streams and evaluate arbitrary NumLang programs.
- [x] **PROD-FUTA2-02**: Drive `MinSpec.nl` specialized against itself using the supercompiler pipeline.
- [x] **PROD-FUTA2-03**: Compile the resulting residual MIR into a native standalone executable binary (`target/release/minspec_cogen.exe`).
- [x] **PROD-FUTA2-04**: Add `--futamura2` CLI command executing the self-specialization and verifying the generated binary.
- [x] **PROD-FUTA2-05**: Verification in `tests/futamura2_binary_tests.rs`: the generated compiler binary compiles 10 distinct NumLang test programs, matching outputs of the primary compiler.

### 50. Strength Reduction in Residual After Loop Collapse (Phase 64) [COMPLETE]
- [x] **STRENGTH-01**: Implement `src/mir/supercompiler/strength_reduce.rs` scanning residual basic blocks for strength reduction opportunities.
- [x] **STRENGTH-02**: Power-of-2 multiplication reduction: replace `mul(x, 2^k)` with `shl(x, k)`.
- [x] **STRENGTH-03**: Near-power-of-2 reduction: replace `mul(x, 2^a ± 2^b)` with shift and add/sub sequences.
- [x] **STRENGTH-04**: Power-of-2 division reduction: replace `div(x, 2^k)` with arithmetic right shifts (`shr`).
- [x] **STRENGTH-05**: Integrate Strassen block recursion for $N \times N$ matrix exponentiation where $N \ge 4$ in `recurrence.rs`.

### 51. CPS Transformation of the Driving Loop (Infinite Stack Safety) (Phase 65) [COMPLETE]
- [x] **CPS-01**: Define explicit driving task structures `enum DriveTask { ProcessNode(ProcessNodeId), HandleTransition(...) }`.
- [x] **CPS-02**: Replace recursive `drive_node` calls with a work-queue trampoline (`VecDeque<DriveTask>`).
- [x] **CPS-03**: Reconstruct ancestor chain paths directly from the process-tree DAG rather than relying on call-stack activation frames.
- [x] **CPS-04**: Enable parallel work-stealing driving using `crossbeam-deque` or scoped threads across independent process branches.
- [x] **CPS-05**: Verification in `tests/deep_recursion_safety_tests.rs`: verify programs with recursion depths > 1,000 drive cleanly without stack overflow, matching single-threaded outputs.

### 52. Incremental Modular Supercompilation with Fine-Grained Invalidation (Phase 66) [COMPLETE]
- [x] **MODCACHE-01**: Extend `SpecializationCache` with an inter-function dependency graph: `HashMap<CacheKey, Vec<CacheKey>>` recording transitive callee dependencies.
- [x] **MODCACHE-02**: Compute composite cache keys incorporating the SHA-256 hashes of the function MIR body and all reachable callee bodies.
- [x] **MODCACHE-03**: Implement fine-grained cache invalidation: when a function changes, invalidate only its upstream callers in the dependency DAG.
- [x] **MODCACHE-04**: Serialize dependency graph and disk cache entries to `.numlang_cache/deps.json` under the `--incremental` CLI flag.
- [x] **MODCACHE-05**: Verification in `tests/incremental_cache_tests.rs`: in a multi-function module, modify a single leaf function and verify that only dependent callers are re-specialized, achieving $\ge 70\%$ cache reuse.

### 53. Total Frontend & Midend Invariant Hardening (Phase 67) [COMPLETE]
- [x] **INV-01**: Replace all 13 `.unwrap()` and 4 `unreachable!()` calls in `src/parser/` with structured `ParseError` diagnostic variants.
- [x] **INV-02**: Replace `.expect()`, `.unwrap()`, and `unreachable!()` calls in `src/typecheck/checker.rs` with `TypeError` variants.
- [x] **INV-03**: Eliminate all `.unwrap()` and `.expect()` calls in `src/ir/lower.rs`, `src/opt/recursion.rs`, `src/opt/inlining.rs`, and `src/ast/hodistill.rs`.
- [x] **INV-04**: Replace Cranelift `unreachable!()` and `GLOBAL_DEOPT_TABLE.write().unwrap()` lock unwraps with structured error handling.

### 54. Recurrence Solver Algorithmic Generality & Intrinsic Name Decoupling (Phase 68) [COMPLETE]
- [x] **REC-01**: Eliminate heuristic string matching on `__numlang_fib` in `src/mir/supercompiler/generalize.rs`.
- [x] **REC-02**: Implement generalized order-2 linear recurrence emission `__numlang_linear_rec2(c1, c2, s0, s1, n)` supporting arbitrary non-zero coefficients.
- [x] **REC-03**: Update `src/codegen/cranelift/mir_emit.rs` to lower `__numlang_linear_rec2` symmetrically without checking for "fib".
- [x] **REC-04**: Update `src/codegen/llvm_backend.rs` to emit general iterative loops for `__numlang_linear_rec2` without special-casing Fibonacci.

### 55. Standalone LLVM Toolchain Driver & Differential Fuzzing Tiering (Phase 69) [COMPLETE]
- [x] **TOOL-01**: Add standalone CLI toolchain driver in `src/codegen/llvm_backend.rs` invoking external `clang`/`llc` on textual LLVM IR when `inkwell` is disabled.
- [x] **TOOL-02**: Un-ignore `tests/differential_correctness_tests.rs` by adapting iteration bounds dynamically for debug vs release modes.
- [x] **TOOL-03**: Add adaptive tiering to `tests/differential_validation_tests.rs` so default debug test runs complete within 120s.
- [x] **TOOL-04**: Configure `tests/differential_fuzz_100k.rs` smoke vs nightly tiering via environment variables.

### 56. Monograph Script Alignment & Repository-Wide Synchronization (Phase 70) [COMPLETE]
- [x] **SYNC-01**: Verify `paper/book/audit_pdf.py` successfully validates monograph page count, TOC, cross-references, and citations.
- [x] **SYNC-02**: Synchronize root `ROADMAP.md` with `.planning/ROADMAP.md` and `.planning/STATE.md`.
- [x] **SYNC-03**: Ensure zero warnings under `cargo clippy --all-targets -- -D warnings` and zero diffs under `cargo fmt -- --check`.
- [x] **SYNC-04**: Push all local commits to remote GitHub `master` branch and verify remote tracking synchronization.


---

## Traceability Matrix

| Requirement | Phase | Status | Target File |
|:---|:---:|:---:|:---|
| CF-01..05 | Phase 3 | Complete | `src/ast.rs`, `src/parser/stmt.rs`, `src/ir/lower.rs` |
| IO-01..04 | Phase 4 | Complete | `src/token.rs`, `src/codegen/cranelift/` |
| TYPE-01..04 | Phase 5 | Complete | `src/typecheck/types.rs`, `src/codegen/cranelift/` |
| STRUCT-01..05 | Phase 6 | Complete | `src/ast.rs`, `src/typecheck/checker.rs`, `src/codegen/cranelift/` |
| MATCH-01..05 | Phase 7 | Complete | `src/ast.rs`, `src/typecheck/checker.rs`, `src/codegen/cranelift/` |
| DIAG-01..04 | Phase 8 | Complete | `src/diagnostic.rs`, `src/main.rs` |
| STD-01..05 | Phase 9 | Complete | `src/typecheck/checker.rs`, `src/codegen/cranelift/` |
| SC-01..05 | Phase 10 | Complete | `src/mir/supercompiler/drive.rs`, `whistle.rs`, `generalize.rs` |
| HOF-01..05 | Phase 11 | Complete | `src/ast.rs`, `src/mir/lower.rs`, `src/mir/supercompiler/drive.rs` |
| GENERIC-01..05 | Phase 12 | Complete | `src/ast.rs`, `src/opt/monomorphize.rs` |
| HEAP-01..05 | Phase 13 | Complete | `src/typecheck/types.rs`, `src/mir/lower.rs`, `src/mir/supercompiler/state.rs` |
| FUTA-PROTO-01..04 | Phase 14 | Complete | `src/stdlib/meta.nl`, `tests/third_futamura_tests.rs` |
| FUZZ-L4-01..04 | Phase 15 | Complete | `fuzz/fuzz_engine.rs`, `proof/NumLangProofs/Semantics.lean` |
| BENCH-HARN-01..04 | Phase 16 | Complete | `bench/harness/runner.py`, `bench/data/results.csv` |
| DOCKER-PEPM-01..04 | Phase 17 | Complete | `docker/Dockerfile`, `paper/main.tex` |
| DIST-MRSC-PROTO-01..04 | Phase 18 | Complete | `src/mir/supercompiler/distill.rs`, `src/mir/supercompiler/mrsc.rs` |
| POLY-VAL-01..04 | Phase 19 | Complete | `src/mir/supercompiler/polyhedral.rs`, `src/mir/supercompiler/validate.rs` |
| RESID-01..03 | Phase 20 | Complete | `src/mir/supercompiler/residualize.rs` |
| MSG-01..03 | Phase 20 | Complete | `src/mir/supercompiler/generalize.rs` |
| DISTILL-01..03 | Phase 21 | Complete | `src/mir/supercompiler/distill.rs` |
| MRSC-01..03 | Phase 22 | Complete | `src/mir/supercompiler/mrsc.rs` |
| POLY-01..03 | Phase 23 | Complete | `src/mir/supercompiler/polyhedral.rs` |
| VALID-01..03 | Phase 24 | Complete | `src/mir/supercompiler/validate.rs` |
| FUTA-01..04 | Phase 25 | Complete | `src/stdlib/minspec.nl` |
| LEAN-01..03 | Phase 26 | Complete | `proof/NumLangProofs/*.lean` |
| BENCH-01..04 | Phase 27 | Complete | `bench/harness/runner.py` |
| PAPER-01..03 | Phase 28 | Complete | `paper/main.tex` |
| PORT-01..05 | Phase 41 | Complete | `src/codegen/cranelift/`, `llvm_backend.rs`, `entry_bench.c` |
| LEAN-04..08 | Phase 42 | Complete | `lean/Supercompiler/*.lean`, `constructive_lean4_phase42_tests.rs` |
| FUTA-05..09 | Phase 43 | Complete | `src/stdlib/minspec.nl`, `third_futamura_tests.rs` |
| FUZZ-01..04 | Phase 44 | Complete | `src/runtime/arena.*`, `escape.rs`, `memory_leak_tests.rs` |
| CODEGEN-01..06 | Phase 45 | Complete | `src/codegen/cranelift/*`, `backend_trait.rs`, `validate.rs` |
| CLEAN-01..05 | Phase 46 | Complete | `src/codegen/cranelift/*`, `src/opt/recursion.rs`, `multi_language_benchmarks.rs` |
| HARDEN-01..05 | Phase 47 | Complete | `src/codegen/cranelift/*`, `src/mir/lower.rs`, `platform_portability_tests.rs` |
| GEN-01..05 | Phase 48 | Complete | `src/opt/recursion.rs`, `src/codegen/llvm_backend.rs`, `tests/structural_generality_tests.rs` |
| DEFUN-01..05 | Phase 49 | Complete | `src/mir/defunctionalize.rs`, `src/mir/supercompiler/*`, `tests/defunctionalize_deforestation_tests.rs` |
| COOPT-01..05 | Phase 50 | Complete | `src/codegen/llvm_backend.rs`, `src/mir/supercompiler/cache.rs`, `tests/supercompiler_llvm_dominance_tests.rs` |
| LAZY-01..05 | Phase 51 | Complete | `src/mir/mod.rs`, `src/mir/thunk_analysis.rs`, `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/distill.rs`, `tests/codata_supercompilation_tests.rs` |
| DEOPT-01..05 | Phase 52 | Complete | `src/mir/speculate.rs`, `src/codegen/cranelift/deopt.rs`, `tests/speculative_deopt_tests.rs` |
| ORACLE-01..05 | Phase 53 | Complete | `src/mir/supercompiler/mrsc_oracle.rs`, `src/mir/supercompiler/mrsc.rs`, `tests/mrsc_oracle_tests.rs` |
| HODIST-01..05 | Phase 54 | Complete | `src/ast/hodistill.rs`, `src/compiler.rs`, `tests/ho_ast_distillation_tests.rs` |
| ILP-01..05 | Phase 55 | Complete | `src/mir/supercompiler/polyhedral_ilp.rs`, `src/mir/supercompiler/polyhedral.rs`, `tests/polyhedral_ilp_tests.rs` |
| OUTLINE-01..05 | Phase 56 | Complete | `src/mir/supercompiler/outliner.rs`, `src/runtime/tier.rs`, `src/compiler.rs`, `tests/tiered_jit_tests.rs` |
| DIFF-01..05 | Phase 57 | Complete | `src/testing/gen.rs`, `src/testing/oracle.rs`, `tests/differential_validation_tests.rs` |
| CLBG-01..05 | Phase 58 | Complete | `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/generalize.rs`, `tests/clbg_correctness_tests.rs` |
| ALG-01..05 | Phase 59 | Complete | `src/mir/supercompiler/term.rs`, `tests/algebraic_reduction_tests.rs` |
| POLYREC-01..05 | Phase 60 | Complete | `src/mir/supercompiler/recurrence.rs`, `src/mir/supercompiler/drive.rs`, `tests/polynomial_recurrence_tests.rs` |
| HASHCONS-01..05 | Phase 61 | Complete | `src/mir/supercompiler/term.rs`, `src/mir/supercompiler/whistle.rs` |
| XFUNC-01..05 | Phase 62 | Complete | `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/recurrence.rs` |
| PROD-FUTA2-01..05 | Phase 63 | Complete | `src/stdlib/minspec.nl`, `src/compiler.rs`, `src/main.rs` |
| STRENGTH-01..05 | Phase 64 | Complete | `src/mir/supercompiler/strength_reduce.rs`, `src/mir/supercompiler/recurrence.rs` |
| CPS-01..05 | Phase 65 | Complete | `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/parallel.rs` |
| MODCACHE-01..05 | Phase 66 | Complete | `src/mir/supercompiler/cache.rs`, `src/compiler.rs`, `src/main.rs` |
| INV-01..04 | Phase 67 | Complete | `src/parser/`, `src/typecheck/`, `src/ir/`, `src/opt/`, `src/ast/`, `src/codegen/cranelift/` |
| REC-01..04 | Phase 68 | Complete | `src/mir/supercompiler/generalize.rs`, `src/codegen/cranelift/mir_emit.rs`, `src/codegen/llvm_backend.rs` |
| TOOL-01..04 | Phase 69 | Complete | `src/codegen/llvm_backend.rs`, `tests/differential_*` |
| SYNC-01..04 | Phase 70 | Complete | `paper/book/audit_pdf.py`, `.planning/*`, `ROADMAP.md` |

