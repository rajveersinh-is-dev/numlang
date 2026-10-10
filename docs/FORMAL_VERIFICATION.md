# NumLang Formal Verification Specification

This document explicitly defines what is mechanized in the Lean 4 formalization, what is modeled, what is cited from literature, and the relationship between the formal models and the Rust compiler.

> **Important Distinction**: The Lean 4 formalization models the core semantics and supercompilation transformations of NumLang. The Rust compiler binary is **not** mechanically extracted from Lean 4. Therefore, **a formal mathematical model of the transformations is verified**, rather than the Rust compiler executable itself.

---

## 1. Why Two Lake Projects Exist in the Repository

NumLang maintains two distinct Lake projects under `lean/` and `proof/`:

1. **`lean/` (`Supercompiler` package)**:
   - Models the control-flow graph (CFG) and Static Single Assignment (SSA) Mid-Level IR fragment: `MirBasicBlock`, `MirFunction`, `MirProgram`, `MirState`, statements, terminators, and execution states.
   - Formalizes transformation relations: `DriveStep`, `MultiDriveStep`, `FoldStep`, `DistillationRelation`, `NoopRemoval`, `EtaReduction`, `lookupRefinement`.
   - Formalizes the top-level composition theorem `supercompiler_sound` (`Main.lean`).
   - Includes `LeanEval.lean` for JSON AST parsing and verification experiments.

2. **`proof/` (`NumLangProofs` package)**:
   - Models the core recursive expression language: `Expr`, `Op`, `Val`, arithmetic operations, and environment lookups.
   - Formalizes both big-step (`BigStep`) and small-step (`SmallStep`, `SmallStepStar`) operational semantics.
   - Mechanizes constructive step-level preservation theorems: constant folding (`smallstep_const_fold`), branch pruning (`smallstep_branch_prune_true`, `smallstep_branch_prune_false`), and recursive call unfolding (`smallstep_call_unfold`).
   - Mechanizes the homeomorphic embedding preorder (`emb_refl`, `emb_trans`, `emb_size_le`) and whistle termination over finite configuration alphabets (`finite_configurations_whistle_terminates`, `no_infinite_whistle_free_path`).

Both projects build independently under standard Lean 4 toolchains (`lake build`). They are separated to decouple the high-level CFG block transformation model from the foundational term-rewriting operational semantics and whistle theory.

---

## 2. Mechanized Verification Scope Matrix

Every file path and theorem below exists in the repository and is verified by `scripts/check_lean_docs.py` against [`docs/LEAN_INVENTORY.md`](LEAN_INVENTORY.md).

| Concept / Result | Lean Source File | Primary Mechanized Theorems | Status | Notes |
|:---|:---|:---|:---:|:---|
| **Multi-Step Reduction Transitivity** | [`lean/Supercompiler/Semantics.lean`](../lean/Supercompiler/Semantics.lean) | `stepstar_trans`, `stepstar_single` | Verified | Relational transitive closure of CFG small-step transitions. |
| **Equivalence Relation Properties** | [`lean/Supercompiler/Semantics.lean`](../lean/Supercompiler/Semantics.lean) | `semantic_equiv_refl`, `semantic_equiv_symm`, `semantic_equiv_trans` | Verified | Reflexivity, symmetry, and transitivity of `SemanticEquivalent`. |
| **Driving Step Preservation** | [`lean/Supercompiler/Preservation.lean`](../lean/Supercompiler/Preservation.lean) | `drive_step_preserves_semantics`, `driving_preserves_semantics` | Verified | Multi-step driving preserves evaluation outcomes on closed configurations. |
| **Distillation & Knot Soundness** | [`lean/Supercompiler/Distillation.lean`](../lean/Supercompiler/Distillation.lean) | `fold_step_preserves_semantics`, `fold_step_is_simulation`, `distillation_preserves_semantics` | Verified | Knot-tying / fold steps simulate ancestor configurations. |
| **Compaction Soundness** | [`lean/Supercompiler/Compaction.lean`](../lean/Supercompiler/Compaction.lean) | `noop_removal_preserves_semantics`, `eta_reduction_preserves_semantics` | Verified | Eliminating dead basic blocks and redundant jumps preserves execution. |
| **Path Refinement Soundness** | [`lean/Supercompiler/Refinement.lean`](../lean/Supercompiler/Refinement.lean) | `refinement_pruning_sound` | Verified | When an interval constraint is contradictory, the branch cannot be taken. |
| **Top-Level Composition** | [`lean/Supercompiler/Main.lean`](../lean/Supercompiler/Main.lean) | `function_transformed_preserves_semantics`, `supercompiler_sound` | Verified | Composes driving, distillation, compaction, and refinement into end-to-end soundness. |
| **Expression Small-Step Preservation** | [`proof/NumLangProofs/Driving.lean`](../proof/NumLangProofs/Driving.lean) | `smallstep_const_fold`, `smallstep_branch_prune_true`, `smallstep_branch_prune_false`, `smallstep_call_unfold` | Verified | Concrete small-step transitions preserve operational semantics on AST expressions. |
| **Homeomorphic Embedding Preorder** | [`proof/NumLangProofs/Termination.lean`](../proof/NumLangProofs/Termination.lean) | `emb_refl`, `emb_trans`, `emb_size_le` | Verified | Reflexivity and transitivity of $\trianglelefteq$, plus size monotonicity. |
| **Whistle Termination on Finite Alphabets** | [`proof/NumLangProofs/Termination.lean`](../proof/NumLangProofs/Termination.lean) | `pigeonhole_seq`, `finite_configurations_whistle_terminates`, `no_infinite_whistle_free_path` | Verified | Sequence pigeonhole principle guarantees no infinite whistle-free path over finite configuration sets. |

---

## 3. What Is Modeled vs. What Is Not Mechanized

### What Is Modeled in Lean 4
1. **Core Operational Semantics**: Small-step and big-step reduction for expressions, local environments, statements, and basic blocks.
2. **Core Transformations**: Constant folding, branch pruning, call unfolding, knot-folding simulation, dead-block elimination, interval pruning.
3. **Termination on Finite Alphabets**: Constructive pigeonhole argument showing that infinite sequences over finite configuration sets encounter duplicate configurations, triggering the whistle via reflexivity.

### What Is Cited from Literature (Not Mechanized)
1. **Kruskal's Tree Theorem for Unbounded Terms**: The well-quasi-ordering (WQO) property of homeomorphic embedding over arbitrary infinite sequences of growing terms is cited from literature (Kruskal 1960, Leuschel 1998, Hamilton 2007). It is not mechanized in Lean.
2. **Higman's Lemma**: WQO on finite alphabet word sequences (Higman 1952).

### What Is NOT Formally Proven in Lean
1. **Rust Binary Extraction**: Cranelift machine code emission and register allocation are not extracted from Lean.
2. **Low-Level Native Calling Conventions**: System ABI, stack layouts, and platform details are tested via integration tests, not proven in Lean.
3. **Micro-architectural Performance**: Benchmark speedups are empirical; no execution time bounds are formalized in Lean.

---

## 4. Trusted Computing Base (TCB)

The formal model proofs rely on:
1. **Lean 4 Kernel**: Consistency and soundness of the Lean 4 proof checker under standard logic (`propext`, `Quot.sound`, `Classical.choice`).
2. **Zero Custom Axioms**: Zero domain-specific axioms or `sorry` admissions exist in the codebase (verified by `scripts/check_lean_axioms.py`).
3. **Host Hardware & OS**: The physical machine executing `lake build`.

---

## 5. In-Tree Translation Validation Engine (`src/mir/supercompiler/validate.rs`)

To validate transformations in the real Rust compiler implementation, NumLang contains an in-tree **Translation Validation Engine** (`src/mir/supercompiler/validate.rs`, 2,326 lines):

### Operational Status: Opt-In Pass
- **Opt-In Flag**: Translation validation is **not enabled by default** in standard compilation (`numlang compile file.nl`). It is an **opt-in pass** invoked when the `--verify-equivalence` (or `--verify`) command-line flag is passed (e.g. `numlang compile --supercompile --verify-equivalence file.nl`).
- **Rationale**: Full bit-level SAT checking adds compilation overhead. Making it opt-in allows developers to run rigorous verification when certifying code without slowing down everyday compilation.

### Architecture
1. **Zero External Solver Dependencies**: Embeds a dedicated QF_BV / CDCL SAT decision procedure within the Rust binary. No dynamic linking to C++ SMT solvers (Z3/CVC5) is required.
2. **Relational Verification Conditions**: Extracts symbolic path conditions connecting entry blocks to terminal returns between the original and residual MIR.
3. **SMT-LIB2 Export**: Under `--verify --emit-smt`, the engine can serialize verification conditions to standard SMT-LIB2 format for external cross-checking.
