# Requirements: NumLang Remediation & Frontier (Phases 20–28)

**Defined:** 2026-09-25  
**Governing Standard:** [`INTEGRITY_RULES.md`](file:///C:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md)  
**Core Value:** Mathematically sound, independently reproducible, world-class supercompilation with zero benchmark cheats, zero fake stubs, zero unproven formal axioms, and 100% genuine algorithmic implementations.

---

## Remediation & Frontier Requirements

### 1. Residualization & Generalization (Phase 20)
- [ ] **RESID-01**: In `src/mir/supercompiler/residualize.rs`, compute state substitution $\theta$ for every knot edge $N_{\text{curr}} \xrightarrow{\text{Knot}} N_{\text{anc}}$ and emit parallel copy variable assignments or block argument passing.
- [ ] **RESID-02**: Remap all `SymTerm::Phi` incoming predecessor basic block IDs to their residual CFG block IDs.
- [ ] **RESID-03**: Verify that supercompiled executables for `nrev.nl`, `append3.nl`, `tree_flip.nl`, and `peano_mul.nl` run to completion with exit code `0` and zero crashes.
- [ ] **MSG-01**: Implement textbook first-order anti-unification (Sørensen & Glück 1995; Plotkin 1970) in `src/mir/supercompiler/generalize.rs`.
- [ ] **MSG-02**: Compute Most-Specific Generalization $\text{msg}(t_1, t_2) = (t_0, \theta_1, \theta_2)$ over symbolic terms when the whistle triggers.
- [ ] **MSG-03**: Generalize state environments component-wise and resume driving with fresh generalization variables.

### 2. Hamilton Global Distillation (Phase 21)
- [ ] **DISTILL-01**: Implement a global process tree representation in `src/mir/supercompiler/distill.rs` modeling call configurations across the entire call graph.
- [ ] **DISTILL-02**: Implement a global whistle and inter-procedural folding mechanism across distinct function definitions.
- [ ] **DISTILL-03**: Verify automated deforestation of composed recursive functions (e.g., `append (append xs ys) zs` $\to$ single-pass 3-argument function without intermediate list allocations).

### 3. Multi-Result Supercompilation (Phase 22)
- [ ] **MRSC-01**: Implement a non-deterministic configuration hypergraph generator in `src/mir/supercompiler/mrsc.rs` branching on driving, folding, and generalization choices.
- [ ] **MRSC-02**: Build a configuration lattice search exploring the space of valid residual programs.
- [ ] **MRSC-03**: Implement Pareto-optimal residualization search extracting optimal programs according to user-selected metrics (code size, step count, branch count).

### 4. Polyhedral Loop & Stencil Deforestation (Phase 23)
- [ ] **POLY-01**: Extract affine iteration domain polyhedra $\{ \vec{i} \mid A \vec{i} + \vec{b} \ge \vec{0} \}$ and access matrices in `src/mir/supercompiler/polyhedral.rs`.
- [ ] **POLY-02**: Compute data dependence distance vectors between producer loops and consumer loops.
- [ ] **POLY-03**: Perform legal affine loop fusion and contract intermediate array buffers to $O(1)$ scalar temporaries or sliding windows.

### 5. Formal SMT-Based Translation Validation (Phase 24)
- [ ] **VALID-01**: Extract Verification Conditions (VCs) and relational path formulas between original and residual MIR CFGs in `src/mir/supercompiler/validate.rs`.
- [ ] **VALID-02**: Encode paths and invariant assertions into QF_BV (quantifier-free bit-vectors) SMT formulas.
- [ ] **VALID-03**: Formally prove simulation preorder over all execution paths under `--verify-equivalence`.

### 6. Genuine Futamura Projections (Phase 25)
- [ ] **FUTA-01**: Implement a self-contained, self-applicable partial evaluator `MinSpec.nl` in NumLang source code (`src/stdlib/minspec.nl`).
- [ ] **FUTA-02**: Verify 1st Futamura projection: $\text{MinSpec}(\text{interp}, \text{prog}) \to \text{prog\_compiled}$.
- [ ] **FUTA-03**: Verify 2nd Futamura projection: $\text{MinSpec}(\text{MinSpec}, \text{interp}) \to \text{compiler}$.
- [ ] **FUTA-04**: Verify 3rd Futamura projection: $\text{MinSpec}(\text{MinSpec}, \text{MinSpec}) \to \text{cogen}$ and prove $\text{cogen}(\text{interp}) \equiv \text{compiler}$.

### 7. Rigorous Lean 4 Formal Verification (Phase 26)
- [ ] **LEAN-01**: Remove `axiom kruskal_tree_theorem` from `proof/NumLangProofs/Termination.lean` and prove termination constructively without axioms.
- [ ] **LEAN-02**: Extend `proof/NumLangProofs/Semantics.lean` to model recursive function environments, heap memory, and control flow.
- [ ] **LEAN-03**: Mechanize the soundness theorem proving that driving, folding, and generalization preserve big-step operational semantics, compiling cleanly with 0 `sorry` and 0 `axiom`s.

### 8. Honest High-Precision Benchmarks (Phase 27)
- [ ] **BENCH-01**: Rewrite `bench/harness/runner.py` to use in-process microsecond hardware performance counter timing across $\ge 10,000$ iterations.
- [ ] **BENCH-02**: Validate process exit codes (`assert returncode == 0`) and report any crashes explicitly as `ERROR`.
- [ ] **BENCH-03**: Fix memory bugs in C baselines (fix `append3` double-free).
- [ ] **BENCH-04**: Benchmark NumLang head-to-head against SPSC and HOSC on canonical literature benchmarks (KMP, Wadler deforestation, Peano multiplication, etc.).

### 9. Paper Rewrite & Artifact Evaluation (Phase 28)
- [ ] **PAPER-01**: Rewrite `paper/main.tex` with automated SHA-256 data pipeline directly populating tables from `bench/data/results.csv`.
- [ ] **PAPER-02**: Accurately describe verified algorithms, honest limitations, and measured speedups without data fabrication.
- [ ] **PAPER-03**: Package a hermetic multi-stage Docker container where `make reproduce` compiles `paper/main.pdf` in one command.

---

## Traceability Matrix

| Requirement | Phase | Status | Target File |
|:---|:---:|:---:|:---|
| RESID-01..03 | Phase 20 | Planned | `src/mir/supercompiler/residualize.rs` |
| MSG-01..03 | Phase 20 | Planned | `src/mir/supercompiler/generalize.rs` |
| DISTILL-01..03 | Phase 21 | Planned | `src/mir/supercompiler/distill.rs` |
| MRSC-01..03 | Phase 22 | Planned | `src/mir/supercompiler/mrsc.rs` |
| POLY-01..03 | Phase 23 | Planned | `src/mir/supercompiler/polyhedral.rs` |
| VALID-01..03 | Phase 24 | Planned | `src/mir/supercompiler/validate.rs` |
| FUTA-01..04 | Phase 25 | Planned | `src/stdlib/minspec.nl` |
| LEAN-01..03 | Phase 26 | Planned | `proof/NumLangProofs/*.lean` |
| BENCH-01..04 | Phase 27 | Planned | `bench/harness/runner.py` |
| PAPER-01..03 | Phase 28 | Planned | `paper/main.tex` |
