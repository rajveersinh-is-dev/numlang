# Requirements: NumLang (v2.0 Phase 2: MemorySSA, Alias Analysis & Mem2Reg)

**Defined:** 2026-09-24
**Core Value:** Eliminate memory bottlenecks by promoting stack variables and struct fields into pure SSA registers, unlocking maximum supercompilation and optimization factors.

## v2.0 Phase 2 Requirements

Requirements for this milestone cycle. Each maps to roadmap phases.

### 1. MemorySSA Representation (MSSA)

- [x] **MSSA-01**: Define `MemoryVersionId` and `MemoryAccess` (`MemoryDef`, `MemoryUse`, `MemoryPhi`) in `src/mir/memory_ssa.rs`.
- [x] **MSSA-02**: Build the MemorySSA graph across basic blocks, connecting statement memory effects to reaching versions.
- [x] **MSSA-03**: Place `MemoryPhi` nodes at CFG join blocks that merge multiple reaching memory versions.

### 2. Points-To & Field-Sensitive Alias Analysis (ALIAS)

- [x] **ALIAS-01**: Implement `AliasAnalysis` and `AliasResult` (`NoAlias`, `MustAlias`, `MayAlias`) in `src/mir/alias.rs`.
- [x] **ALIAS-02**: Statically prove disjointness for distinct stack locals and non-overlapping root allocations.
- [x] **ALIAS-03**: Implement field-sensitive path queries ensuring distinct struct fields (`p.x` vs `p.y`) report `NoAlias`.
- [x] **ALIAS-04**: Disjoint constant array index analysis (`arr[0]` vs `arr[1]` report `NoAlias`).

### 3. Mem2Reg SSA Promotion Engine (M2R)

- [ ] **M2R-01**: Identify promote-candidate non-aliased local variables and struct fields.
- [ ] **M2R-02**: Compute iterated dominance frontiers (IDF) using `src/mir/dominance.rs` for optimal $\phi$-placement.
- [ ] **M2R-03**: Execute dominator-tree variable renaming, replacing loads with SSA registers and eliminating stores.
- [ ] **M2R-04**: Implement Redundant Load Elimination (RLE) and Dead Store Elimination (DSE) over memory places.

### 4. CLI & Comprehensive Testing (TEST)

- [ ] **TEST-01**: Add `--emit-memory-ssa` flag to `src/main.rs` to visualize MemorySSA tokens.
- [ ] **TEST-02**: Create `tests/memory_ssa_tests.rs` covering basic promotion, loop $\phi$-handling, field alias queries, and DSE/RLE.
- [ ] **TEST-03**: Verify zero Clippy warnings (`-- -D warnings`) and 100% green pass rate across all 37+ test suites.

## Out of Scope

| Feature | Reason |
|---|---|
| Whole-program interprocedural heap alias analysis | Deferred to Phase 3/4 when heap pointers and dynamic allocators are introduced. |
| Raw C unchecked pointer casting | NumLang's memory model is safe; raw pointers are reserved for Phase 3 C ABI interop. |

## Traceability

| Requirement | Phase | Status |
|---|---|---|
| MSSA-01 | Phase 2.1 | Complete |
| MSSA-02 | Phase 2.1 | Complete |
| MSSA-03 | Phase 2.1 | Complete |
| ALIAS-01 | Phase 2.2 | Complete |
| ALIAS-02 | Phase 2.2 | Complete |
| ALIAS-03 | Phase 2.2 | Complete |
| ALIAS-04 | Phase 2.2 | Complete |
| M2R-01 | Phase 2.3 | Pending |
| M2R-02 | Phase 2.3 | Pending |
| M2R-03 | Phase 2.3 | Pending |
| M2R-04 | Phase 2.3 | Pending |
| TEST-01 | Phase 2.4 | Pending |
| TEST-02 | Phase 2.4 | Pending |
| TEST-03 | Phase 2.4 | Pending |
