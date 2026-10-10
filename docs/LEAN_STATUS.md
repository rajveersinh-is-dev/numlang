# NumLang Formal Verification & Lean 4 Proof Status

Date: 2026-10-10  
Auditor / Maintainer: Team Formal Verification  
Status: **VERIFIED & MACHINE-CHECKED**  

---

## 1. Executive Summary & Verification Guarantees

NumLang maintains two mechanized Lean 4 formalization packages in the repository:
1. `lean/`: Operational semantics, small-step determinism, and transformation preservation for the core supercompiler (`lakefile.lean`, 10 build targets).
2. `proof/`: Driving step soundness, simulation preorder, and termination lemmas (`lakefile.lean`, 6 build targets).

### Integrity Guarantee
- **Zero `sorry`** declarations anywhere in `lean/` or `proof/`.
- **Zero custom / unproven `axiom`** declarations anywhere in `lean/` or `proof/`.
- **100% reproducible build** verified locally and in GitHub Actions CI (`lean.yml`).

```
$ cd lean; lake build
Build completed successfully (10 jobs).

$ cd ../proof; lake build
Build completed successfully (6 jobs).
```

---

## 2. Axiom Inventory

Every theorem in the repository was inspected for logical dependency axioms using Lean 4's environment introspection.

| Axiom Name | Origin | Category | Purpose | Status |
|:---|:---|:---|:---|:---:|
| `propext` | Lean 4 Core Kernel | Standard Logic | Propositional Extensionality: equivalent propositions are equal | **STANDARD** |
| `Quot.sound` | Lean 4 Core Kernel | Standard Logic | Quotient Soundness: relates quotient equivalence classes | **STANDARD** |
| `Classical.choice` | Lean 4 Core Kernel | Classical Logic | Non-constructive choice for existential witnesses | **STANDARD** |
| *Domain-Specific Axioms* | **None** | Domain Logic | Custom compiler or language axioms | **ZERO (0)** |

**Finding**: NumLang introduces zero non-standard axioms. All proofs are strictly constructive or use standard classical type theory.

---

## 3. Mechanized Theorems Inventory

### Package `lean/` (`Supercompiler`)

| File | Primary Theorems / Definitions | Description | Proof Size | Status |
|:---|:---|:---|:---:|:---:|
| `Supercompiler/Semantics.lean` | `step`, `eval`, `step_deterministic` | Constructive small-step reduction relation; evaluation function; determinism theorem | 370 lines | **PROVEN** |
| `Supercompiler/Preservation.lean` | `eval_preservation`, `step_star_preservation` | Multi-step reduction semantic preservation | 62 lines | **PROVEN** |
| `Supercompiler/Refinement.lean` | `simulation_preorder`, `trace_equivalence` | Simulation relations between unoptimized and residual configurations | 95 lines | **PROVEN** |
| `Supercompiler/Distillation.lean` | `unfold_sound`, `fold_sound`, `intermediate_elimination` | Consumer-producer intermediate data structure deforestation soundness | 74 lines | **PROVEN** |
| `Supercompiler/Compaction.lean` | `prune_unreachable_branch`, `compact_path` | Control flow branch pruning and linear path compaction soundness | 58 lines | **PROVEN** |

### Package `proof/` (`NumLangProofs`)

| File | Primary Theorems / Definitions | Description | Proof Size | Status |
|:---|:---|:---|:---:|:---:|
| `NumLangProofs/Semantics.lean` | `State`, `Step`, `Deterministic` | Memory states and deterministic relational evaluation semantics | 120 lines | **PROVEN** |
| `NumLangProofs/Driving.lean` | `DriveStep`, `drive_preserves_eval` | Symbolic driving step operational equivalence | 280 lines | **PROVEN** |
| `NumLangProofs/Termination.lean` | `pigeonhole_seq` | Pigeonhole Principle for infinite sequences into finite lists | 48 lines | **PROVEN** |
| `NumLangProofs/Termination.lean` | `finite_alphabet_good_sequence` | Finite-alphabet homeomorphic embedding good sequence theorem | 11 lines | **PROVEN** |
| `NumLangProofs/Termination.lean` | `no_infinite_whistle_free_path` | Absence of infinite whistle-free branches in finite-alphabet process trees | 12 lines | **PROVEN** |

---

## 4. Exact Mathematical Scope & Literature Boundaries

To maintain computational honesty and avoid overclaims (Audit Findings F6 and F8), the exact boundaries of formal verification are delineated as follows:

### What Is Mechanically Proven in Lean 4
1. **Operational Determinism**: If $S \to S_1$ and $S \to S_2$, then $S_1 = S_2$.
2. **Semantic Preservation of Driving**: Driving steps preserve evaluation outcomes on closed instances.
3. **Pigeonhole Termination Lemma**: Any infinite sequence over a finite set of program configuration symbols contains an index pair $i < j$ with $\text{seq}(i) = \text{seq}(j)$, which by reflexivity triggers the homeomorphic embedding whistle ($\text{seq}(i) \trianglelefteq \text{seq}(j)$), preventing infinite unrolling.

### What Is Cited From Literature
1. **Kruskal's Tree Theorem for Unbounded Terms**: For infinite sequences over general algebraic term algebras with infinite terms, well-quasi-ordering (WQO) is guaranteed by Kruskal's Tree Theorem (Kruskal 1960, Nash-Williams 1963). NumLang formalizes the finite-alphabet sequence theorem directly, citing Kruskal's WQO theorem from the supercompilation literature (Leuschel 1998, Hamilton 2007).
2. **Higman's Lemma**: Well-quasi-ordering of words over finite alphabets (Higman 1952).

### What Is Verified via Translation Validation & Testing (Not Extracted from Lean)
1. **Cranelift Native Machine Code Codegen**: The Rust compiler binary is not mechanically extracted from Lean 4.
2. **Translation Validation Engine (`src/mir/supercompiler/validate.rs`)**: Bridges the Rust implementation by verifying simulation equivalence between original MIR and residual CFG via an in-tree QF_BV / CDCL SAT solver.
3. **Differential Correctness**: Mechanically tested across 100% of example programs against the reference interpreter in `tests/differential_correctness_tests.rs`.
