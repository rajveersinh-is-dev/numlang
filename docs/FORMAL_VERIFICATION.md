# NumLang Formal Verification Specification

This document explicitly delineates what is mathematically proven in the Lean 4 formalization, what is modeled, what is not proven, and the trusted computing base.

> **Important Distinction**: The Lean 4 formalization models the core semantics and supercompilation transformations of NumLang. The Rust compiler binary is **not** mechanically extracted from Lean 4. Therefore, **a formal model of the algorithm is verified**, rather than the Rust compiler executable itself.

---

## 1. Scope & Verification Matrix

| Theorem / Concept | Lean Module | Status | Verification Type | Notes |
|:---|:---|:---:|:---:|:---|
| **StepStar Operational Semantics** | `lean/Supercompiler/Semantics.lean` | Verified | Machine-Checked | Constructive small-step reduction relation. Zero `sorry`, zero `axiom`. |
| **Deterministic Reduction** | `lean/Supercompiler/Deterministic.lean` | Verified | Machine-Checked | For all states $S$, if $S \to S_1$ and $S \to S_2$, then $S_1 = S_2$. |
| **Drive Step Soundness** | `lean/Supercompiler/DriveSoundness.lean` | Verified | Machine-Checked | Symbolic driving steps preserve small-step operational equivalence. |
| **Constant Folding Soundness** | `lean/Supercompiler/Transformations.lean` | Verified | Machine-Checked | $e \to^* v \implies \text{eval}(\text{fold}(e)) = \text{eval}(e)$. |
| **Branch Pruning Soundness** | `lean/Supercompiler/Transformations.lean` | Verified | Machine-Checked | Condition evaluation accurately eliminates unreachable control flow arms. |
| **Knot Folding Equivalence** | `lean/Supercompiler/FoldSoundness.lean` | Verified | Machine-Checked | Recursive loop knot formation preserves trace equivalence under substitution. |
| **Whistle Quasi-Order** | `lean/Supercompiler/Termination.lean` | Verified | Machine-Checked | Homeomorphic embedding on finite-signature symbolic terms is well-quasi-ordered. |

---

## 2. What Is Modeled vs What Is Not Proven

### What Is Modeled
1. **Core SSA Intermediate Representation**: An abstract AST/MIR covering arithmetic expressions, branching, variables, and recursive function definitions.
2. **Symbolic Driving & Unfolding**: Tree expansion over symbolic states with free variable constraints.
3. **Folding & Generalization**: Structural matching against ancestor nodes and substitution generation.

### What Is NOT Formally Proven
1. **Rust Binary Extraction**: The Cranelift machine code emission and Rust runtime are not extracted from Lean.
2. **Low-Level Native Calling Conventions**: System ABI, stack frames, register allocation, and Windows x64 ABI are verified via differential testing, not formal Lean proofs.
3. **Hardware Micro-architectural Timing**: Algorithmic speedups are empirical; no formal hardware-latency upper bounds are established in Lean.

---

## 3. Trusted Computing Base (TCB)

The formal claims rely on:
1. **Lean 4 Kernel**: Type theory consistency and soundness of the Lean 4 proof checker (`propext`, `Quot.sound`, `Classical.choice`).
2. **Host Hardware & OS**: The physical environment executing the proof checker and test harnesses.

All proofs in `lean/` build cleanly with **zero `sorry`** declarations and **zero unproven `axiom`** assumptions.
