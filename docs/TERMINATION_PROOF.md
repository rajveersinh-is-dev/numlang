# Formal Correctness and Certified Termination of the NumLang Supercompiler

## Abstract
This document provides the mathematical justification and machine-checked Lean 4 formalization for the operational soundness and termination properties of the NumLang supercompiler models. We establish:
1. **Driving Soundness**: Symbolic driving preserves big-step operational semantics on closed expressions (`theorem driving_soundness`), and small-step rewrite rules preserve semantics (`theorem smallstep_const_fold`, `theorem smallstep_call_unfold`).
2. **Certified Whistle Termination on Finite Alphabets**: Over an assumed finite alphabet of configuration symbols, any infinite path contains duplicate nodes by the sequence pigeonhole principle (`theorem pigeonhole_seq`), which triggers the whistle by reflexivity of homeomorphic embedding (`theorem emb_refl`), guaranteeing that no infinite whistle-free path exists (`theorem no_infinite_whistle_free_path`). For infinite sequences of arbitrarily growing terms, well-quasi-ordering via Kruskal's Tree Theorem is cited from literature (Kruskal 1960), not mechanized in Lean.

All mechanized theorems have been verified in Lean 4 without custom axioms or `sorry` placeholders in `proof/` and `lean/`.

---

## 1. NumLang Core Formal Operational Semantics

NumLang Core is formalized as an inductive calculus with integer arithmetic, conditionals, local let-bindings, and de Bruijn indexed variables:

$$\begin{aligned}
\text{Op} &::= \text{add} \mid \text{sub} \mid \text{mul} \mid \text{div} \mid \text{eq} \mid \text{lt} \\
e &::= n \mid x_i \mid \text{bin}(\text{op}, e_1, e_2) \mid \text{cond}(c, t, f) \mid \text{letIn}(e_1, e_2) \\
v &::= \text{intVal}(n) \\
\rho &::= [v_0, v_1, \dots, v_{k-1}]
\end{aligned}$$

The big-step evaluation judgment $\rho \vdash e \Downarrow v$ is defined inductively in `proof/NumLangProofs/Semantics.lean`:

$$\frac{}{\rho \vdash n \Downarrow \text{intVal}(n)} \quad (\text{Lit})$$

$$\frac{\rho[i] = v}{\rho \vdash x_i \Downarrow v} \quad (\text{Var})$$

$$\frac{\rho \vdash e_1 \Downarrow \text{intVal}(n_1) \quad \rho \vdash e_2 \Downarrow \text{intVal}(n_2) \quad \text{evalOp}(\text{op}, n_1, n_2) = \text{some}(n_3)}{\rho \vdash \text{bin}(\text{op}, e_1, e_2) \Downarrow \text{intVal}(n_3)} \quad (\text{Bin})$$

$$\frac{\rho \vdash c \Downarrow \text{intVal}(n_c) \quad n_c \neq 0 \quad \rho \vdash t \Downarrow v}{\rho \vdash \text{cond}(c, t, f) \Downarrow v} \quad (\text{CondTrue})$$

$$\frac{\rho \vdash c \Downarrow \text{intVal}(0) \quad \rho \vdash f \Downarrow v}{\rho \vdash \text{cond}(c, t, f) \Downarrow v} \quad (\text{CondFalse})$$

$$\frac{\rho \vdash e_1 \Downarrow v_1 \quad (v_1 :: \rho) \vdash e_2 \Downarrow v_2}{\rho \vdash \text{letIn}(e_1, e_2) \Downarrow v_2} \quad (\text{LetIn})$$

---

## 2. Mechanized Driving and Soundness

Driving is the core symbolic reduction operation in Turchin-style supercompilation. It evaluates constant redexes and deterministic branches before expanding or generalizing non-terminating loops.

### Theorem 1 (Driving Soundness)
For all environments $\rho$, expressions $e$, and values $v$:
$$\rho \vdash e \Downarrow v \implies \forall e', \text{drive}(e) = \text{some}(e') \implies \rho \vdash e' \Downarrow v$$

**Proof (Machine-checked in `proof/NumLangProofs/Driving.lean`)**:
By structural induction on the big-step derivation tree $\mathcal{D} :: \rho \vdash e \Downarrow v$:
- **Base cases** (`lit`, `var`): Driving returns `none`, so the implication holds vacuously.
- **Binary Operation** (`bin`): If both operands are literals $n_1, n_2$ and $\text{evalOp}(\text{op}, n_1, n_2) = \text{some}(n_3)$, driving yields $\text{lit}(n_3)$. By inversion, $v = \text{intVal}(n_3)$, which satisfies $\rho \vdash \text{lit}(n_3) \Downarrow \text{intVal}(n_3)$ by rule `Lit`.
- **Conditional** (`cond`):
  - If condition is a static literal $n_c \neq 0$, driving yields branch $t$. From the premise of `CondTrue`, $\rho \vdash t \Downarrow v$ holds directly.
  - If condition is a static literal $0$, driving yields branch $f$. From the premise of `CondFalse`, $\rho \vdash f \Downarrow v$ holds directly.
$\blacksquare$

---

## 3. Homeomorphic Embedding and the Whistle

To prevent infinite loops during symbolic execution and tree expansion, the supercompiler checks the **homeomorphic embedding relation** $s \trianglelefteq t$ between any new configuration $t$ and all its ancestor configurations $s$.

### Definition (Homeomorphic Embedding)
1. **Variable Reflexivity**: $x_i \trianglelefteq x_j$ for all indices $i, j$.
2. **Literal Reflexivity**: $n \trianglelefteq n$.
3. **Subterm Diving**:
   - $s \trianglelefteq t_1 \lor s \trianglelefteq t_2 \implies s \trianglelefteq \text{bin}(\text{op}, t_1, t_2)$
   - $s \trianglelefteq c \lor s \trianglelefteq t \lor s \trianglelefteq f \implies s \trianglelefteq \text{cond}(c, t, f)$
   - $s \trianglelefteq e_1 \lor s \trianglelefteq e_2 \implies s \trianglelefteq \text{letIn}(e_1, e_2)$
4. **Functor Coupling**:
   - $s_1 \trianglelefteq t_1 \land s_2 \trianglelefteq t_2 \implies \text{bin}(\text{op}, s_1, s_2) \trianglelefteq \text{bin}(\text{op}, t_1, t_2)$
   - $c_1 \trianglelefteq c_2 \land t_1 \trianglelefteq t_2 \land f_1 \trianglelefteq f_2 \implies \text{cond}(c_1, t_1, f_1) \trianglelefteq \text{cond}(c_2, t_2, f_2)$
   - $s_1 \trianglelefteq t_1 \land s_2 \trianglelefteq t_2 \implies \text{letIn}(s_1, s_2) \trianglelefteq \text{letIn}(t_1, t_2)$

### Lemma 1 (Preorder)
Homeomorphic embedding $\trianglelefteq$ is reflexive and transitive:
$$\forall e, e \trianglelefteq e$$
$$\forall e_1, e_2, e_3, e_1 \trianglelefteq e_2 \land e_2 \trianglelefteq e_3 \implies e_1 \trianglelefteq e_3$$

**Proof (Machine-checked in `proof/NumLangProofs/Termination.lean`)**:
- Reflexivity follows by straightforward structural induction on $e$.
- Transitivity follows by structural induction on the second derivation $e_2 \trianglelefteq e_3$ generalizing over $e_1$, with case inversion on $e_1 \trianglelefteq e_2$ in the coupling cases. $\blacksquare$

---

## 4. Finite-Configuration Termination and Literature WQO Foundations

### Theorem 2 (Whistle Termination on Finite Configurations -- Mechanized)
In the NumLang supercompiler, whenever a configuration $t_k$ embeds an ancestor $t_a$ ($t_a \trianglelefteq t_k$ for $a < k$), the **whistle blows**, halting driving along that path and triggering loop folding or Most Specific Generalization (MSG).

Therefore, any infinite expansion path would have to be **whistle-free**:
$$\forall i < j, \quad \neg (t_i \trianglelefteq t_j)$$

**Proof (Machine-checked in `proof/NumLangProofs/Termination.lean`, 0 axioms, 0 sorry)**:
In a supercompiler process tree operating on a source program, configurations are drawn from a finite alphabet of configuration symbols (`InAlphabet alphabet path`). By the constructive sequence pigeonhole principle (`pigeonhole_seq`), in any sequence $\pi$ over a finite alphabet of length $N$, among the first $N+1$ entries there exist distinct indices $i < j$ such that:
$$\pi(i) = \pi(j)$$
By reflexivity of homeomorphic embedding (`emb_refl`), $\pi(i) \trianglelefteq \pi(j)$ holds (`finite_alphabet_good_sequence`, `finite_configurations_whistle_terminates`).

Assume for contradiction there exists an infinite whistle-free path $\pi$ over `alphabet` (`WhistleFreePath path`). The good sequence property yields $i < j$ with $\pi(i) \trianglelefteq \pi(j)$, directly contradicting the whistle-free premise $\forall i < j, \neg (\pi(i) \trianglelefteq \pi(j))$. Therefore, `¬ ∃ path, WhistleFreePath path` holds (`no_infinite_whistle_free_path`). $\blacksquare$

### Mathematical Foundation: Kruskal's Tree Theorem (Cited from Literature)
For general term algebras where configurations can grow to unbounded sizes without restriction to an a priori finite alphabet of terms, termination of homeomorphic embedding whistles rests on **Kruskal's Tree Theorem** (Kruskal 1960):

> **Theorem (Kruskal's Tree Theorem for Terms, Kruskal 1960)**: Let $\Sigma$ be a finite ranked signature of term constructors, and let $\mathcal{T}(\Sigma)$ be the set of finite trees over $\Sigma$. If the set of leaf labels is well-quasi-ordered, then the homeomorphic embedding relation $\trianglelefteq$ over $\mathcal{T}(\Sigma)$ is a **Well-Quasi-Ordering (WQO)**. That is, in any infinite sequence of expressions $t_0, t_1, t_2, \dots \in \mathcal{T}(\Sigma)$, there exist indices $i < j$ such that $t_i \trianglelefteq t_j$.

> **Literature Citation**: Kruskal's full theorem for unbounded terms is cited from literature (Kruskal 1960, Nash-Williams 1963, Leuschel 1998, Hamilton 2007) and is not mechanized in Lean 4. What is mechanized in NumLang is the finite-configuration pigeonhole whistle termination argument (`finite_configurations_whistle_terminates`, `no_infinite_whistle_free_path`) and the preorder properties of homeomorphic embedding (`emb_refl`, `emb_trans`, `emb_size_le`).

---

## 5. Verification Instructions
To independently verify the entire proof suite:
```bash
./scripts/verify_proofs.sh
```
Or on Windows PowerShell:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify_proofs.ps1
```
The Lake build tool checks all definitions and proofs with zero warnings, zero errors, and zero admitted propositions.
