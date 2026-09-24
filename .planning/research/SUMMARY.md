# Project Research Summary

**Project:** NumLang (v2.0 Phase 2: MemorySSA & Alias Analysis)
**Domain:** Middle-End SSA Compiler Optimization
**Researched:** 2026-09-24
**Confidence:** HIGH

## Executive Summary

Phase 2 builds directly upon the newly merged Phase 1 MIR Control Flow Graph (`src/mir/`). While Phase 1 established basic blocks, place projections, and dominance frontiers, statements currently read and write variables through explicit `Place` references. In general systems code, this representation leaves local variables and struct fields trapped in memory, preventing downstream passes from performing aggressive loop vectorization, scalar optimizations, and closed-form derivation.

The recommended architectural approach is the industry-standard **MemorySSA + Mem2Reg pipeline** utilized by LLVM and GCC. By introducing a field-sensitive alias analysis engine and modeling memory state as versioned tokens (`MemoryDef`, `MemoryUse`, `MemoryPhi`), NumLang can cleanly identify non-aliased stack allocations, compute iterated dominance frontiers via the existing `src/mir/dominance.rs`, and promote local stack structures into pure SSA registers.

The key risk in SSA construction is incorrect $\phi$-placement across loop headers and over-conservative alias analysis. By adopting Cytron's classic IDF algorithm and enforcing strict field-sensitivity for struct projections, NumLang will achieve optimal register promotion with zero regressions across its 37 existing test suites.

## Key Findings

### Recommended Stack
- **Core Engine**: Pure Rust middle-end modules in `src/mir/` (`alias.rs`, `memory_ssa.rs`, `mem2reg.rs`).
- **Data Structures**: Versioned `MemoryAccess` tokens paired with Cytron iterated dominance frontier computation.
- **Backend Bridge**: Directly benefits Cranelift AOT and the algebraic supercompiler by exposing clean scalar def-use chains.

### Expected Features
- **Must Have (Table Stakes)**:
  - `src/mir/alias.rs`: Field-sensitive alias queries (`NoAlias`, `MustAlias`, `MayAlias`).
  - `src/mir/memory_ssa.rs`: Explicit `MemoryDef`, `MemoryUse`, and `MemoryPhi` representation.
  - `src/mir/mem2reg.rs`: Full Mem2Reg promotion of non-escaping stack locals and struct fields into SSA block arguments / temporaries.
- **Competitive Differentiators**:
  - Redundant Load Elimination (RLE) across straight-line code and dominator trees.
  - Dead Store Elimination (DSE) for shadowed stack writes.
  - Exposing promoted loops to NumLang's algebraic supercompiler for closed-form collapse.

### Architecture Approach
1. `src/mir/alias.rs`: Query engine for disjoint place roots and struct field offsets.
2. `src/mir/memory_ssa.rs`: Linear-time memory dependency graph.
3. `src/mir/mem2reg.rs`: Dominator-tree SSA variable renaming and load-store elimination.
4. Comprehensive testing in `tests/memory_ssa_tests.rs`.

### Critical Pitfalls
1. **Incomplete Reaching Definitions at Loop Headers**: Solved by two-pass SSA variable stack renaming over Cytron IDF.
2. **Coarse Alias Collapsing**: Solved by field-sensitivity distinguishing `p.x` from `p.y`.
3. **Premature Dead Store Removal across Calls**: Solved by treating function calls as global memory barriers / memory uses.
