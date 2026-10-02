# Requirements: NumLang Remediation & Frontier (Phases 20–28)

**Defined:** 2026-09-25  
**Governing Standard:** [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md)  
**Core Value:** Mathematically sound, independently reproducible, world-class supercompilation with zero benchmark cheats, zero fake stubs, zero unproven formal axioms, and 100% genuine algorithmic implementations.

---

## Remediation & Frontier Requirements (Phases 20–28) [COMPLETE]

### 1. Residualization & Generalization (Phase 20)
- [x] **RESID-01**: In `src/mir/supercompiler/residualize.rs`, compute state substitution $\theta$ for every knot edge $N_{\text{curr}} \xrightarrow{\text{Knot}} N_{\text{anc}}$ and emit parallel copy variable assignments or block argument passing.
- [x] **RESID-02**: Remap all `SymTerm::Phi` incoming predecessor basic block IDs to their residual CFG block IDs.
- [x] **RESID-03**: Verify that supercompiled executables for `nrev.nl`, `append3.nl`, `tree_flip.nl`, and `peano_mul.nl` run to completion with exit code `0` and zero crashes.
- [x] **MSG-01**: Implement textbook first-order anti-unification (Sørensen & Glück 1995; Plotkin 1970) in `src/mir/supercompiler/generalize.rs`.
- [x] **MSG-02**: Compute Most-Specific Generalization $\text{msg}(t_1, t_2) = (t_0, \theta_1, \theta_2)$ over symbolic terms when the whistle triggers.
- [x] **MSG-03**: Generalize state environments component-wise and resume driving with fresh generalization variables.

### 2. Hamilton Global Distillation (Phase 21)
- [x] **DISTILL-01**: Implement a global process tree representation in `src/mir/supercompiler/distill.rs` modeling call configurations across the entire call graph.
- [x] **DISTILL-02**: Implement a global whistle and inter-procedural folding mechanism across distinct function definitions.
- [x] **DISTILL-03**: Verify automated deforestation of composed recursive functions (e.g., `append (append xs ys) zs` $\to$ single-pass 3-argument function without intermediate list allocations).

### 3. Multi-Result Supercompilation (Phase 22)
- [x] **MRSC-01**: Implement a non-deterministic configuration hypergraph generator in `src/mir/supercompiler/mrsc.rs` branching on driving, folding, and generalization choices.
- [x] **MRSC-02**: Build a configuration lattice search exploring the space of valid residual programs.
- [x] **MRSC-03**: Implement Pareto-optimal residualization search extracting optimal programs according to user-selected metrics (code size, step count, branch count).

### 4. Polyhedral Loop & Stencil Deforestation (Phase 23)
- [x] **POLY-01**: Extract affine iteration domain polyhedra $\{ \vec{i} \mid A \vec{i} + \vec{b} \ge \vec{0} \}$ and access matrices in `src/mir/supercompiler/polyhedral.rs`.
- [x] **POLY-02**: Compute data dependence distance vectors between producer loops and consumer loops.
- [x] **POLY-03**: Perform legal affine loop fusion and contract intermediate array buffers to $O(1)$ scalar temporaries or sliding windows.

### 5. Formal SMT-Based Translation Validation (Phase 24)
- [x] **VALID-01**: Extract Verification Conditions (VCs) and relational path formulas between original and residual MIR CFGs in `src/mir/supercompiler/validate.rs`.
- [x] **VALID-02**: Encode paths and invariant assertions into QF_BV (quantifier-free bit-vectors) SMT formulas.
- [x] **VALID-03**: Formally prove simulation preorder over all execution paths under `--verify-equivalence`.

### 6. Genuine Futamura Projections (Phase 25)
- [x] **FUTA-01**: Implement a self-contained, self-applicable partial evaluator `MinSpec.nl` in NumLang source code (`src/stdlib/minspec.nl`).
- [x] **FUTA-02**: Verify 1st Futamura projection: $\text{MinSpec}(\text{interp}, \text{prog}) \to \text{prog\_compiled}$.
- [x] **FUTA-03**: Verify 2nd Futamura projection: $\text{MinSpec}(\text{MinSpec}, \text{interp}) \to \text{compiler}$.
- [x] **FUTA-04**: Verify 3rd Futamura projection: $\text{MinSpec}(\text{MinSpec}, \text{MinSpec}) \to \text{cogen}$ and prove $\text{cogen}(\text{interp}) \equiv \text{compiler}$.

### 7. Rigorous Lean 4 Formal Verification (Phase 26)
- [x] **LEAN-01**: Remove `axiom kruskal_tree_theorem` from `proof/NumLangProofs/Termination.lean` and prove termination constructively without axioms.
- [x] **LEAN-02**: Extend `proof/NumLangProofs/Semantics.lean` to model recursive function environments, heap memory, and control flow.
- [x] **LEAN-03**: Mechanize the soundness theorem proving that driving, folding, and generalization preserve big-step operational semantics, compiling cleanly with 0 `sorry` and 0 `axiom`s.

### 8. Honest High-Precision Benchmarks (Phase 27)
- [x] **BENCH-01**: Rewrite `bench/harness/runner.py` to use in-process microsecond hardware performance counter timing across $\ge 10,000$ iterations.
- [x] **BENCH-02**: Validate process exit codes (`assert returncode == 0`) and report any crashes explicitly as `ERROR`.
- [x] **BENCH-03**: Fix memory bugs in C baselines (fix `append3` double-free).
- [x] **BENCH-04**: Benchmark NumLang head-to-head against SPSC and HOSC on canonical literature benchmarks (KMP, Wadler deforestation, Peano multiplication, etc.).

### 9. Paper Rewrite & Artifact Evaluation (Phase 28)
- [x] **PAPER-01**: Rewrite `paper/main.tex` with automated SHA-256 data pipeline directly populating tables from `bench/data/results.csv`.
- [x] **PAPER-02**: Accurately describe verified algorithms, honest limitations, and measured speedups without data fabrication.
- [x] **PAPER-03**: Package a hermetic multi-stage Docker container where `make reproduce` compiles `paper/main.pdf` in one command.

---

## Adversarial Remediation Requirements (Phases 41–45) [COMPLETE]

### 10. Win32 Decoupling & POSIX Codegen (Phase 41)
- [x] **PORT-01**: In `src/codegen/cranelift_backend.rs`, conditionally declare Win32 APIs only when targeting Windows. On non-Windows platforms, declare standard C library `exit` and `write`.
- [x] **PORT-02**: In `src/codegen/llvm_backend.rs`, dynamically target the host triple and emit conditional `exit` and `write` signatures for non-Windows targets.
- [x] **PORT-03**: Update `src/codegen/entry_bench.c` with `#ifdef _WIN32` portability branch using `clock_gettime(CLOCK_MONOTONIC)` on POSIX.
- [x] **PORT-04**: Implement checked integer arithmetic (`checked_mul`, `checked_add`) and non-zero divisor guard in `src/mir/supercompiler/generalize.rs` for order-2 linear recurrences.
- [x] **PORT-05**: Create `tests/platform_portability_tests.rs` verifying absence of undefined Windows symbols on non-Windows builds.

### 11. Constructive Lean 4 Operational Proofs (Phase 42)
- [x] **LEAN-04**: In `lean/Supercompiler/Semantics.lean`, define small-step `Step` relation and transitive closure `StepStar`. Define `TerminatesWith` and re-anchor `Evaluates`.
- [x] **LEAN-05**: Prove simulation preservation lemmas (`step_blocks_equiv`, `stepstar_blocks_equiv`, `terminates_blocks_equiv`, `semantic_equiv_of_blocks_equiv`).
- [x] **LEAN-06**: Eliminate circular hypotheses from `Preservation.lean` (`DriveStep`), `Compaction.lean` (`NoopRemoval`, `EtaReduction`), and `Distillation.lean` (`FoldStep`).
- [x] **LEAN-07**: Prove end-to-end `supercompiler_sound` in `Main.lean`.
- [x] **LEAN-08**: Provide test suite `tests/constructive_lean4_phase42_tests.rs` verifying 0 `sorry`, 0 `axiom`, and successful `lake build`.

### 12. Full Futamura Projections (Phase 43)
- [x] **FUTA-05**: Expand `src/stdlib/minspec.nl` with full recursive AST representations (`SpecExpr` with literals, variables, binary operators, if-conditions, function calls).
- [x] **FUTA-06**: Implement 1st Futamura projection: $\text{Specialize}(\text{interp}, \text{prog}) \to \text{target\_prog}$.
- [x] **FUTA-07**: Implement 2nd Futamura projection: $\text{Specialize}(\text{specialize}, \text{interp}) \to \text{compiler}$.
- [x] **FUTA-08**: Implement 3rd Futamura projection: $\text{Specialize}(\text{specialize}, \text{specialize}) \to \text{cogen}$.
- [x] **FUTA-09**: Verify structural divergence $\text{AST}(\text{cogen}) \neq \text{AST}(\text{compiler}) \neq \text{AST}(\text{interp})$ and 0 copy-paste duplication.

### 13. Scoped Arena & Memory Safety (Phase 44)
- [x] **FUZZ-01**: Implement arena allocator runtime in `src/runtime/arena.rs` and `src/runtime/arena.c` with chunk management, peak tracking, and reset functions (`__nl_arena_create`, `__nl_arena_alloc`, `__nl_arena_reset`, `__nl_loop_reset`).
- [x] **FUZZ-02**: Integrate arena fast-path into `src/codegen/cranelift/` and `src/codegen/cranelift_backend.rs` (`__nl_arena_cur`, `__nl_arena_end`).
- [x] **FUZZ-03**: Implement loop escape analysis (`should_reset_loop_iteration`) in backend and `residualize.rs` to emit `__nl_loop_reset()` latch at loop back-edges for non-escaping allocations.
- [x] **FUZZ-04**: Verify 50,000-iteration memory bounds and zero memory corruption in `tests/memory_leak_tests.rs`.

### 14. Monolith Decomposition & SMT Loop Validation (Phase 45)
- [x] **CODEGEN-01**: Partition Cranelift backend into `src/codegen/cranelift/` (`mod.rs`, `abi.rs`, `intrinsics.rs`, `escape.rs`, `ast_stmt.rs`, `ast_expr.rs`, `mir_emit.rs`). Retain `src/codegen/cranelift_backend.rs` as a thin forwarding shim.
- [x] **CODEGEN-02**: Enforce strict file size bound: no file exceeds 2,500 lines.
- [x] **CODEGEN-03**: Define `BackendCompiler` trait in `src/codegen/backend_trait.rs` and implement for `CraneliftCompiler` and `LlvmCompiler`.
- [x] **CODEGEN-04**: Replace all bare `.unwrap()` / `.expect()` in codegen with structured `CodegenError` variants.
- [x] **CODEGEN-05**: Purge deprecated `src/opt/supercompiler/` directory.
- [x] **CODEGEN-06**: Implement $k$-induction loop translation validation in `src/mir/supercompiler/validate.rs`.

---

## Traceability Matrix

| Requirement | Phase | Status | Target File |
|:---|:---:|:---:|:---|
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

