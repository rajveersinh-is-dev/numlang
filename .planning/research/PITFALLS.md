# Pitfalls Research: MemorySSA & Alias Analysis

**Domain:** SSA Construction and Memory Dependency Tracking
**Researched:** 2026-09-24
**Confidence:** HIGH

## Critical Pitfalls & Prevention Strategies

### Pitfall 1: Incorrect Phi Placement in Loops (The Incomplete Reaching Definition Bug)
- **The Issue**: In loops with back-edges, a memory load inside the loop header may read either the initial value from the pre-header or an updated value from the loop latch. If $\phi$-nodes are placed prematurely or without considering back-edges, the compiler may substitute the pre-loop initial value into the loop body, causing silent data corruption.
- **Prevention**: Use standard Cytron iterated dominance frontiers (`IDF`) and maintain two-pass SSA variable stacks: forward traversal records placeholder block arguments, and latch edges fill in operand arguments.

### Pitfall 2: Overly Conservative Pointer Aliasing
- **The Issue**: If the alias analyzer treats all array indices `arr[i]` and `arr[j]` or distinct struct fields `p.x` and `p.y` as `MayAlias`, `Mem2Reg` fails to promote struct fields into registers.
- **Prevention**: Enforce field-sensitivity: struct fields with distinct names (`p.x` vs `p.y`) on the same root base are statically known to have distinct non-overlapping offsets (`NoAlias`). Constant array indices (`arr[0]` vs `arr[1]`) must also report `NoAlias`.

### Pitfall 3: Dead Store Elimination Across Memory Barriers / Calls
- **The Issue**: Removing a store to an array or struct because there are no following direct loads in the same block, but the data is subsequently passed to a function call (`Call`) or returned.
- **Prevention**: Any `Call` or `Return` must act as a `MemoryUse` on all reachable escaping memory locations, preventing premature dead-store elimination.

### Pitfall 4: Combinatorial MemorySSA Graph Blowup
- **The Issue**: Generating a `MemoryPhi` at every block join even when memory was never modified along one or more paths leads to $O(B \cdot V)$ memory tokens.
- **Prevention**: Pruned SSA form: only insert `MemoryPhi` at join blocks that lie in the iterated dominance frontier of actual `MemoryDef` operations.
