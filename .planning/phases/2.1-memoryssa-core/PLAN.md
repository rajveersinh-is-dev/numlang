# Phase 2.1 Plan: MemorySSA Core & Graph Construction

## Goal
Implement a single-token versioned MemorySSA representation and graph builder for NumLang MIR. This models memory state transitions as SSA tokens (`MemoryDef`, `MemoryUse`, `MemoryPhi`), linking all memory read/write operations to reaching memory versions.

## Tasks

1. **CFG Helper & Loop Support in MIR**:
   - In `src/mir/lower.rs`:
     - Add `loop_stack: Vec<(BasicBlockId, BasicBlockId)>` to `MirBuilder` so `TypedStmt::Break` and `TypedStmt::Continue` properly branch to exit and loop header.
   - In `src/mir/mod.rs`:
     - Add `compute_cfg(blocks: &[MirBasicBlock])` returning `(predecessors, successors)`.

2. **MemorySSA Module (`src/mir/memory_ssa.rs`)**:
   - Define:
     - `MemoryVersionId(pub usize)`
     - `MemoryDef { id, incoming, place, block, statement_index }`
     - `MemoryUse { reaching, place, block, statement_index }`
     - `MemoryPhi { id, block, incoming: Vec<(BasicBlockId, MemoryVersionId)> }`
     - `MemoryAccess`: Enum of Def, Use, Phi
     - `MemorySSA`: Graph storing `block_phis`, `stmt_accesses`, `block_entry_versions`, `block_exit_versions`, and def/phi maps.
   - Implement `MemorySsaBuilder`:
     - Place `MemoryPhi` at CFG join points (blocks with >1 predecessors / iterated dominance frontier).
     - Dominator tree traversal with SSA version stack renaming.
     - Fill in phi incoming operands from predecessor exit versions.
     - Simplify trivial phis if all operands match.
     - Provide display/pretty-print for MemorySSA inspection.

3. **Register module in `src/mir/mod.rs`**:
   - `pub mod memory_ssa;`

4. **Unit Tests & Verification**:
   - Add unit tests in `tests/mir_tests.rs`:
     - Sequential block memory version progression (`v0 -> v1 -> v2`).
     - Branch / merge with `MemoryPhi`.
     - While loop memory versioning and loop header $\phi$.
   - Verify all tests pass (`cargo test --tests`) and zero clippy warnings (`cargo clippy --all-targets -- -D warnings`).
