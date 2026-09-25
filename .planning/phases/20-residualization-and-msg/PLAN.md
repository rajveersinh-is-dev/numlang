# Phase 20: Fix Core Residualization, Back-Edge Knot Transfers & Textbook MSG

## Goal
Eliminate runtime crashes and invalid CFG generation in supercompiled binaries by properly emitting variable state assignments on knot back-edges (`ProcessEdge::Knot`), remapping Phi node basic block IDs to residual blocks, and implementing textbook Most-Specific Generalization (MSG).

## Context & Audit Findings
In `honest_review.md`, the audit revealed:
1. When `residualize.rs` encounters `ProcessEdge::Knot(anc_id)`, it emits a bare jump `Terminator::Branch { target: node_to_block[anc_id] }`. It does not emit statements to update loop state variables or pass block arguments, resulting in uninitialized/stale variables or infinite loops with unchanged registers.
2. `SymTerm::Phi` preserves the `BasicBlockId`s of the original unoptimized program, which are completely mismatched with the freshly numbered residual basic blocks.
3. As a result, heap-allocating and recursive benchmarks (`nrev`, `append3`, `tree_flip`, `peano_mul`) crash with segfaults or memory access violations at runtime, causing Windows Error Reporting (WER) delays of 430–485 ms.
4. `generalize.rs` relies on an ad-hoc polynomial curve-fitting heuristic instead of proper anti-unification.

## Implementation Tasks

### Task 1: Knot State Transfer & Parallel Copy Generation (`src/mir/supercompiler/residualize.rs`)
- For every knot edge $(N_{\text{curr}} \xrightarrow{\text{Knot}} N_{\text{anc}})$, compare the active state environment at $N_{\text{curr}}$ with the ancestor state environment at $N_{\text{anc}}$.
- Identify all variables $x \in \text{dom}(\sigma_{\text{anc}})$ whose symbolic terms differ between $N_{\text{curr}}$ and $N_{\text{anc}}$.
- Emit assignments to update each variable to its new evaluated value prior to the `Terminator::Branch` to $N_{\text{anc}}$.
- Resolve cyclic assignments using temporary scratch variables to prevent register clobbering.

### Task 2: Residual Block ID Remapping for Phi Nodes (`src/mir/supercompiler/residualize.rs`)
- Maintain a bidirectional mapping between process tree transitions and residual `BasicBlockId`s: `(ProcessNodeId, ProcessNodeId) -> BasicBlockId`.
- When emitting `SymTerm::Phi(incoming, ty)`, remap each incoming branch `(old_bb, term)` to the corresponding residual predecessor `BasicBlockId`.

### Task 3: Textbook Most-Specific Generalization (MSG) (`src/mir/supercompiler/generalize.rs`)
- Implement first-order anti-unification (Sørensen & Glück 1995; Plotkin 1970).
- Function signature:
  ```rust
  pub fn anti_unify(
      t1: SymTermId,
      t2: SymTermId,
      interner: &mut TermInterner,
      subst1: &mut HashMap<String, SymTermId>,
      subst2: &mut HashMap<String, SymTermId>,
  ) -> SymTermId
  ```
- If $t_1 = t_2$, return $t_1$.
- If $t_1 = f(a_1, \dots, a_n)$ and $t_2 = f(b_1, \dots, b_n)$ with identical functor $f$, return $f(\text{anti\_unify}(a_1, b_1), \dots, \text{anti\_unify}(a_n, b_n))$.
- Otherwise, check if a generalization variable $v$ already exists for pair $(t_1, t_2)$; if so return $v$, else generate fresh variable $v_{\text{fresh}}$, recording $v_{\text{fresh}} \mapsto t_1$ in $\text{subst}_1$ and $v_{\text{fresh}} \mapsto t_2$ in $\text{subst}_2$.
- Integrate component-wise across symbolic states when the whistle blows.

### Task 4: Verification Gate & Crash Elimination
- Compile and execute:
  - `bench/numlang/nrev.nl`
  - `bench/numlang/append3.nl`
  - `bench/numlang/tree_flip.nl`
  - `bench/numlang/peano_mul.nl`
- Verify with `--supercompile`:
  - Process exit code is `0`.
  - Stdout matches unoptimized run.
  - Runtime does not trigger WER delay (sub-millisecond execution).
- Run `cargo test --tests` to verify 100% green across all test suites.
- Run `cargo clippy --all-targets -- -D warnings` with zero warnings.

## Success Criteria
- [ ] Zero crashes on heap-allocating and recursive supercompiled benchmarks.
- [ ] Correct parallel copy emission on all knot edges.
- [ ] Valid CFG with correctly mapped Phi predecessor blocks.
- [ ] Passing unit and integration tests.
