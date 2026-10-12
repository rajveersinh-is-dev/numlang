# Lean 4 Formal Coverage & Rust Compiler Mapping

**Date:** 2026-10-11  
**Phase:** Phase E (Lean Honesty & Coverage)  
**Status:** Mechanized Operational Models (17 files, 1,699 lines, 45 theorems/lemmas, 0 `sorry`, 0 custom axioms)  

---

## 1. Architectural Scope & Model Boundary

NumLang maintains a formal verification subsystem in Lean 4 consisting of two packages:
- `lean/` (Lake package `Supercompiler`): Basic-block SSA Mid-Level IR operational semantics and process tree transformation relations.
- `proof/` (Lake package `NumLangProofs`): Expression-level operational semantics, homeomorphic embedding preorders, and finite-alphabet pigeonhole termination.

### Honest Boundary Disclosure
The Lean 4 proofs verify **abstract operational models** of the compiler transformations. Specifically:
1. **Model Equivalence**: The formalization proves that if a process tree transformation satisfies the Lean relations (`DriveStep`, `FoldStep`, `DistillationRelation`), semantic equivalence is preserved:
   $$\forall \sigma, \llbracket e \rrbracket \sigma = \llbracket e' \rrbracket \sigma$$
2. **Translation Gap**: The production Rust binary compiler (`src/`) is **not** mechanically extracted from Lean 4.
3. **Defense-in-Depth**: Because Lean models the abstract semantics rather than compiler backend code generation, production safety is established through:
   - **Formal In-Tree Translation Validation** (`src/mir/supercompiler/validate.rs`, 2,326 lines): SMT QF_BV bit-vector equivalence verification of actual input MIR vs residual CFG.
   - **Differential Correctness Testing** (`tests/differential_correctness_tests.rs`): 10,000+ randomized programs cross-validated between interpreter, baseline Cranelift, and supercompiled binaries.

---

## 2. Formal Relation to Rust Implementation Mapping

The table below provides a 1-to-1 mapping between the relations mechanized in Lean 4 and their corresponding implementations in the NumLang compiler:

| Lean Formal Relation | Lean File & Theorems | Rust Subsystem & File | Concrete Rust Implementation | Operational Transformation | Formal Soundness Guarantee |
|:---|:---|:---|:---|:---|:---|
| **`DriveStep`** | [`lean/Supercompiler/Preservation.lean`](../lean/Supercompiler/Preservation.lean)<br>`drive_step_preserves_semantics`<br>`driving_preserves_semantics` | Supercompiler Driver<br>[`src/mir/supercompiler/drive.rs`](../src/mir/supercompiler/drive.rs) | `SupercompilerDriver::step_node`<br>`SupercompilerDriver::eval_rvalue` | Symbolic evaluation of SSA statements, constant folding of arithmetic expressions, inlining calls up to budget. | $\forall \sigma, \text{StepStar}(f, \sigma) \iff \text{StepStar}(f', \sigma)$ |
| **`FoldStep`** | [`lean/Supercompiler/Distillation.lean`](../lean/Supercompiler/Distillation.lean)<br>`fold_step_preserves_semantics`<br>`fold_step_is_simulation` | Knot Tying Engine<br>[`src/mir/supercompiler/drive.rs`](../src/mir/supercompiler/drive.rs) | `SupercompilerDriver::try_fold`<br>`ProcessTree::tie_knot` | Recognizing previously visited configurations $\alpha \equiv \beta$, folding loop back-edges, and generating recursive knot residuals. | Bidirectional weak simulation between unfolded loop body and folded knot call. |
| **`DistillationRelation`** | [`lean/Supercompiler/Distillation.lean`](../lean/Supercompiler/Distillation.lean)<br>`distillation_preserves_semantics` | Distillation Engine<br>[`src/mir/supercompiler/distill.rs`](../src/mir/supercompiler/distill.rs) | `DistillationEngine::distill_process_tree`<br>`DistillationEngine::fold_matching_subtrees` | Global Hamilton distillation: folding recursive consumer-producer pairs into single recursive residual loops. | Semantic equivalence of distilled process trees. |
| **`Refinement`** | [`lean/Supercompiler/Refinement.lean`](../lean/Supercompiler/Refinement.lean)<br>`refinement_pruning_sound` | Path Constraints & SMT<br>[`src/mir/supercompiler/generalize.rs`](../src/mir/supercompiler/generalize.rs) | `PathConstraint::refine`<br>`LinearConstraint::is_satisfiable` | Interval constraint narrowing along conditional branches; eliminating impossible branches from residual CFGs. | If interval predicate is false, branch can never execute. |
| **`MRSCSelection`** | [`lean/Supercompiler/MRSC.lean`](../lean/Supercompiler/MRSC.lean)<br>`mrsc_selection_preserves_semantics` | Multi-Result Engine<br>[`src/mir/supercompiler/mrsc.rs`](../src/mir/supercompiler/mrsc.rs) | `MultiResultEngine::explore_and_select`<br>`ParetoFrontier::select_optimal` | Pareto-optimal selection from non-dominated candidate process trees generated via multi-result exploration. | Selecting any candidate from sound derivation set preserves input semantics. |
| **`Compaction`** | [`lean/Supercompiler/Compaction.lean`](../lean/Supercompiler/Compaction.lean)<br>`noop_removal_preserves_semantics`<br>`eta_reduction_preserves_semantics` | MIR Tree Compaction<br>[`src/mir/supercompiler/mod.rs`](../src/mir/supercompiler/mod.rs)<br>[`src/mir/supercompiler/compact.rs`](../src/mir/supercompiler/compact.rs) | `compact_process_tree`<br>`compact_mir_function` | Removal of identity assignments ($x \leftarrow x$), unreachable basic blocks, and redundant unconditional jumps. | Semantic equivalence under statement compaction and $\eta$-reduction. |
| **`HomeomorphicEmbedding`** | [`proof/NumLangProofs/Termination.lean`](../proof/NumLangProofs/Termination.lean)<br>`emb_refl`<br>`emb_trans`<br>`emb_size_le` | Whistle Detector<br>[`src/mir/supercompiler/whistle.rs`](../src/mir/supercompiler/whistle.rs) | `is_embedded`<br>`state_embeds` | Detection of structural expression growth ($e_1 \trianglelefteq e_2$) along tree paths to trigger generalization before infinite loops. | Reflexivity, transitivity, and size monotonicity of homeomorphic embedding preorder. |
| **`PigeonholeTermination`** | [`proof/NumLangProofs/Termination.lean`](../proof/NumLangProofs/Termination.lean)<br>`pigeonhole_seq`<br>`finite_configurations_whistle_terminates` | Driver Termination Guard<br>[`src/mir/supercompiler/drive.rs`](../src/mir/supercompiler/drive.rs) | `SupercompilerDriver::run`<br>`driver.config.max_inline_nodes` | Bounding unfolding depth and halting search when configurations repeat over finite state subsets. | Non-existence of infinite whistle-free derivation paths over finite configuration sets. |

---

## 3. Axiom Disclosures & Mechanical Introspection

Every headline theorem in both Lean packages is introspected via `#print axioms` in `lean/AxiomCheck.lean` and `proof/AxiomCheck.lean`. The complete output is preserved in [`docs/evidence/round2/lean_axioms.txt`](../docs/evidence/round2/lean_axioms.txt):

```text
--- Axiom output for lean/AxiomCheck.lean ---
'Supercompiler.const_fold_stmt_equiv' depends on axioms: [propext, Quot.sound]
'Supercompiler.drive_step_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.driving_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.fold_step_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.distillation_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.refinement_pruning_sound' depends on axioms: [propext]
'Supercompiler.mrsc_selection_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.noop_removal_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.eta_reduction_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.function_transformed_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.supercompiler_sound' depends on axioms: [propext, Quot.sound]
--- Axiom output for proof/AxiomCheck.lean ---
'NumLang.driving_correctness' depends on axioms: [propext]
'NumLang.driving_soundness' depends on axioms: [propext]
'NumLang.driving_equiv' depends on axioms: [propext]
'NumLang.unfold_call_equiv' depends on axioms: [propext]
'NumLang.folding_equiv' depends on axioms: [propext]
'NumLang.generalization_equiv' depends on axioms: [propext]
'NumLang.smallstep_const_fold' does not depend on any axioms
'NumLang.smallstep_branch_prune_true' does not depend on any axioms
'NumLang.smallstep_call_unfold' does not depend on any axioms
'NumLang.emb_refl' does not depend on any axioms
'NumLang.emb_trans' does not depend on any axioms
'NumLang.pigeonhole_seq' depends on axioms: [propext, Classical.choice, Quot.sound]
'NumLang.finite_alphabet_good_sequence' depends on axioms: [propext, Classical.choice, Quot.sound]
'NumLang.finite_configurations_whistle_terminates' depends on axioms: [propext, Classical.choice, Quot.sound]
'NumLang.no_infinite_whistle_free_path' depends on axioms: [propext, Classical.choice, Quot.sound]
```

### Verification Findings:
1. **Zero `sorry` Placeholders**: 0 instances anywhere in the codebase.
2. **Zero Custom / Domain Axioms**: All proofs reduce to standard Lean foundational logic (`propext`, `Quot.sound`, `Classical.choice`). Several key theorems (e.g., `smallstep_const_fold`, `emb_refl`, `emb_trans`) are purely constructive, depending on **zero** axioms.
3. **Zero Duplicate Theorems**: Cross-package scanning confirms zero duplicate theorem identifiers between `lean/` and `proof/`.
