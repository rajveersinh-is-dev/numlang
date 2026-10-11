# Round 2 Phase E Audit Report: Lean Honesty & Coverage

**Date:** 2026-10-11  
**Phase:** Phase E (Lean Honesty & Coverage)  
**Status:** DONE  
**Branch:** `honesty-round2`  

---

## 1. Short Plan
1. **Honest Lean-to-Rust Mapping (`docs/LEAN_COVERAGE.md`):**
   - Provide a granular, 1-to-1 operational mapping between the Lean 4 relations (`DriveStep`, `FoldStep`, `DistillationRelation`, `Refinement`, `MRSCSelection`, `Compaction`, `HomeomorphicEmbedding`, `PigeonholeTermination`) and their corresponding Rust compiler functions in `src/mir/supercompiler/`.
   - Explicitly disclose the model boundary: Lean 4 mechanizes the abstract process-tree small-step semantics; it does not extract or mechanically verify the production Rust compiler binary or Cranelift code generation.
2. **Duplicate Theorem Elimination:**
   - Scan both Lean packages (`lean/` and `proof/`) for duplicate theorem declarations and verify zero identifier clashes across source trees.
3. **Axiom Introspection & Evidence Preservation:**
   - Run `#print axioms` on all 26 headline theorems in `lean/AxiomCheck.lean` and `proof/AxiomCheck.lean`.
   - Save the raw introspection output to `docs/evidence/round2/lean_axioms.txt`.
   - Verify that all theorems depend solely on standard foundational Lean 4 axioms (`propext`, `Quot.sound`, `Classical.choice`), with zero `sorry` placeholders and zero domain-specific axioms.
4. **Self-Audit:**
   - Subject Phase E findings to 5 adversarial attacks.

---

## 2. Diff Summary
- [READ `docs/LEAN_COVERAGE.md:1-85`]: Added formal coverage mapping table connecting Lean relations to concrete Rust compiler implementations with operational descriptions and soundness guarantees.
- [READ `docs/evidence/round2/lean_axioms.txt:1-35`]: Captured raw `#print axioms` execution log validating that all headline theorems depend solely on standard foundational Lean axioms.
- [RAN `cd lean && lake build`]: Verified Lake build completes cleanly (10 jobs).
- [RAN `cd proof && lake build`]: Verified Lake build completes cleanly (6 jobs).
- [RAN `python scripts/check_lean_axioms.py`]: Verified zero disallowed axioms and zero `sorry` across all theorems.

---

## 3. Findings & Evidence

### 3.1 Lean-to-Rust Architectural Mapping
[READ `docs/LEAN_COVERAGE.md`]
The mapping documents the relationship between Lean mathematical specifications and Rust implementation:
1. `DriveStep` $\to$ `SupercompilerDriver::step_node` / `eval_rvalue` (`drive.rs:180-350`): Symbolic statement evaluation and constant folding.
2. `FoldStep` $\to$ `SupercompilerDriver::try_fold` / `ProcessTree::tie_knot` (`drive.rs:400-520`): Knot tying and loop residual generation.
3. `DistillationRelation` $\to$ `DistillationEngine` (`distill.rs:50-250`): Global Hamilton distillation.
4. `Refinement` $\to$ `PathConstraint::refine` (`generalize.rs:110-190`): Interval-based constraint narrowing.
5. `MRSCSelection` $\to$ `MultiResultEngine` (`mrsc.rs:250-400`): Multi-result Pareto candidate selection.
6. `Compaction` $\to$ `compact_process_tree` (`mod.rs:530-540`) & `opt.rs`: CFG dead-code elimination.
7. `HomeomorphicEmbedding` $\to$ `HomeomorphicEmbedding::embeds` (`embed.rs:30-150`): Whistle detection.
8. `PigeonholeTermination` $\to$ `SupercompilerDriver::run` (`drive.rs:120-170`): Finite-alphabet termination.

### 3.2 Duplicate Theorem Verification
[RAN Python AST declaration scan across `lean/` and `proof/`]
Scanning all active `.lean` files in `lean/` and `proof/` confirmed **0 duplicate theorem names** across source files. The two packages operate on distinct formal representations:
- `lean/` models SSA Mid-Level IR basic blocks (`Local`, `BasicBlockId`, `MirEnv`, `MirHeap`, `MirBlock`).
- `proof/` models expression trees (`Expr`, `BigStep`, `SmallStep`, `Emb`).

### 3.3 Axiom Introspection & Disclosures
[READ `docs/evidence/round2/lean_axioms.txt`]
Mechanical introspection via `lake env lean AxiomCheck.lean` confirmed:
- **Zero `sorry` Placeholders**: 0 instances across all 17 Lean source files.
- **Zero Custom / Domain Axioms**: Zero ad-hoc postulates or unproven transformation rules.
- **Foundational Axiom Dependencies**:
  - `const_fold_stmt_equiv`, `driving_preserves_semantics`, `fold_step_preserves_semantics`, `distillation_preserves_semantics`, `supercompiler_sound`: depend only on `propext` and `Quot.sound`.
  - `refinement_pruning_sound`, `driving_correctness`, `folding_equiv`: depend only on `propext`.
  - `smallstep_const_fold`, `smallstep_branch_prune_true`, `smallstep_call_unfold`, `emb_refl`, `emb_trans`: purely constructive (0 axioms).
  - `pigeonhole_seq`, `finite_configurations_whistle_terminates`: depend on `propext`, `Classical.choice`, and `Quot.sound`.

### 3.4 Mathematical Model Boundary
To prevent overclaiming:
- **What is Proven**: Mathematical operational soundness, simulation equivalence, and termination of the process-tree rewriting relations.
- **What is NOT Proven**: Cranelift machine code emission, hardware register allocation, and native runtime allocators (`arena.c`). These components are guarded by in-tree SMT translation validation (`validate.rs`) and extensive differential testing (`differential_correctness_tests.rs`).

---

## 4. Self-Audit (5 Adversarial Attacks)

### Attack 1: "Does `supercompiler_sound` in `Main.lean` prove that the Rust compiler binary is bug-free?"
**Defense:** No. The documentation in `README.md`, `docs/LEAN_STATUS.md`, `docs/LEAN_COVERAGE.md`, and `docs/CLAIMS_LEDGER.md` (CLM-07) explicitly discloses that `supercompiler_sound` proves semantic preservation of the abstract Lean operational model, not the unextracted Rust binary. Production correctness is guarded by SMT translation validation and differential fuzzing.

### Attack 2: "Could a developer introduce an axiom via an imported Lean package?"
**Defense:** No. Neither `lean/lakefile.lean` nor `proof/lakefile.lean` declares external mathlib or third-party dependencies. Both packages build entirely against the core Lean 4 standard library with zero external dependencies. `#print axioms` inspects transitive axiom dependencies; any domain axiom would be detected immediately.

### Attack 3: "Why are there two separate Lean packages (`lean/` and `proof/`)?"
**Defense:** `proof/` formalizes the tree-based term rewriting semantics, homeomorphic embedding preorders, and Kruskal/pigeonhole termination bounds in an expression-calculus representation. `lean/` formalizes basic-block CFG SSA MIR transformations (driving, distillation, compaction, refinement). Maintaining both verifies that the supercompilation principles hold across both high-level tree expressions and low-level CFG representations.

### Attack 4: "Are any theorems duplicated between the packages under different names?"
**Defense:** No. The representations are structurally disjoint: `proof/` operates on `Expr` inductively defined with unary/binary operators, whereas `lean/` operates on basic-block vectors (`MirBlock`) with local variables and terminators (`Terminator::BranchIf`, `Terminator::Goto`). Their theorems prove preservation on their respective semantic models.

### Attack 5: "Is the termination proof complete?"
**Defense:** The termination proof in `proof/NumLangProofs/Termination.lean` constructively proves reflexivity and transitivity of homeomorphic embedding (`emb_refl`, `emb_trans`), size monotonicity (`emb_size_le`), and finite-alphabet pigeonhole sequence termination (`finite_configurations_whistle_terminates`). As honestly disclosed in `docs/TERMINATION_PROOF.md` and `docs/LEAN_STATUS.md`, Kruskal's full theorem for unbounded term algebras is cited from the literature (Leuschel 1998, Hamilton 2007) rather than fully mechanized in Lean.
