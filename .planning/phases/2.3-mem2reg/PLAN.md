# Phase 2.3 Plan: Mem2Reg Promotion Engine & Memory Optimizations

## Goal
Implement the Mem2Reg promotion pass (`src/mir/mem2reg.rs`) and memory optimizations (Redundant Load Elimination and Dead Store Elimination). This transforms non-escaping stack variables and individual struct fields into SSA values passed through basic block arguments ($\phi$-nodes) placed at Iterated Dominance Frontiers (IDF).

## Tasks

1. **Candidate Identification (M2R-01)**:
   - Identify memory places that qualify for SSA promotion:
     - Pure local variables with no address taken.
     - Struct fields (e.g. `p.x`, `p.y`) where the struct is local and only accessed through known fields.
   - Separate candidate places from places that cannot be promoted (e.g. dynamic array indices `arr[i]`).

2. **Iterated Dominance Frontier (IDF) Calculation (M2R-02)**:
   - For each candidate place $P$:
     - Find all basic blocks containing definitions (stores/assignments to $P$).
     - Compute the Iterated Dominance Frontier:
       $IDF(S) = \text{fixed-point of } DF(S \cup IDF(S))$
       using `dominance_frontiers` from `src/mir/dominance.rs`.
     - In each block $B \in IDF(S)$, insert block argument $\phi$ for $P$.

3. **Dominator Tree Renaming Pass (M2R-03)**:
   - Maintain a scoped stack of current reaching SSA values for each candidate place $P$.
   - Traverse the dominator tree in depth-first order:
     - At block entry: if block has block argument / $\phi$ for $P$, push the block argument as current reaching value.
     - For each statement in block:
       - If statement reads $P$ (`Rvalue::Use(P)`): replace read of $P$ with current reaching SSA value (Rvalue::Use of the reaching temp).
       - If statement writes $P$ (`dest == P`): set reaching value to the assigned RHS, and eliminate the redundant memory store!
     - For each CFG successor block $S$:
       - If $S$ has block arguments ($\phi$-nodes), pass the current reaching value along the corresponding branch terminator.
     - Recurse to dominator tree children.
     - On exit from block, pop definitions pushed in this block.

4. **Redundant Load Elimination (RLE) & Dead Store Elimination (DSE) (M2R-04)**:
   - RLE: If a place $P$ is loaded multiple times without any intervening aliasing store (`ModRefResult::Mod`), forward the first load to subsequent loads.
   - DSE: If a store to place $P$ is immediately overwritten by another store to $P$ without any intervening load or function return, remove the earlier store.

5. **Register Module in `src/mir/mod.rs`**:
   - `pub mod mem2reg;`

6. **Unit Tests & Verification**:
   - Add unit tests in `tests/mir_tests.rs`:
     - Test single-block scalar promotion (store + load eliminated).
     - Test branching block IDF $\phi$-node generation and argument passing.
     - Test field-sensitive struct promotion (`p.x` and `p.y` promoted independently).
     - Test Dead Store Elimination (dead store before overwrite removed).
     - Test Redundant Load Elimination (second load forwarded).
   - Ensure all 37+ test suites pass and clippy has 0 warnings.
