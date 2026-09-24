# Phase 2.1 Summary: MemorySSA Core & Graph Construction

## Outcome
Implemented industrial-grade MemorySSA token representation and graph construction pass for NumLang MIR.

## Key Changes
1. **CFG & Loop Control in MIR (`src/mir/mod.rs`, `src/mir/lower.rs`)**:
   - Implemented `Terminator::successors` and `compute_cfg(blocks)` for general CFG predecessor/successor mapping.
   - Added `loop_stack` to `MirBuilder`, completing proper branching for `TypedStmt::Break` (to loop exit) and `TypedStmt::Continue` (to loop header).
2. **MemorySSA Engine (`src/mir/memory_ssa.rs`)**:
   - Defined `MemoryVersionId`, `MemoryDef`, `MemoryUse`, `MemoryPhi`, and `MemoryAccess`.
   - Built dominator-tree traversal with SSA version stack propagation for memory def-use chains.
   - Inserted `MemoryPhi` nodes at CFG join points merging memory states from multiple predecessor branches.
   - Implemented trivial phi simplification.
   - Added `MemorySSA::display` for visualization of memory tokens.
3. **Verification**:
   - Added unit tests in `tests/mir_tests.rs` for linear memory versioning, branching $\phi$-joins, and while-loop header $\phi$-nodes.
   - All tests passing, 0 clippy warnings (`-- -D warnings`).
