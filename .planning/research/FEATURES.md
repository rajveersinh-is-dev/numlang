# Features Research: MemorySSA & Alias Analysis

**Domain:** Compiler Optimization & Middle-End SSA Transformation
**Researched:** 2026-09-24
**Confidence:** HIGH

## Feature Taxonomy

### Table Stakes (Must Have for Phase 2)
- **FEAT-01: MemorySSA Core Nodes (`MemoryAccess`)**:
  - `MemoryDef`: Produced by statements that mutate memory (stores into place projections, field assignments, impure calls). Holds a unique `MemoryVersionId`.
  - `MemoryUse`: Produced by statements that read memory (loads from place projections, field reads). References the reaching `MemoryVersionId`.
  - `MemoryPhi`: Synthesized at CFG merge blocks to unify reaching memory versions from predecessor blocks.
- **FEAT-02: Flow-Sensitive Points-To & Alias Analysis (`AliasAnalysis`)**:
  - Distinguishes between `NoAlias`, `MustAlias`, and `MayAlias`.
  - Handles distinct local variables on the stack as provably disjoint (`NoAlias`).
  - Field sensitivity: `struct Point { x, y }` guarantees that `p.x` and `p.y` have distinct offsets and cannot alias each other.
- **FEAT-03: Mem2Reg Promotion Engine (`mem2reg`)**:
  - Automatically identifies all non-address-taken local variables and scalar struct fields.
  - Inserts $\phi$-nodes at iterated dominance frontiers using `src/mir/dominance.rs`.
  - Renames memory loads into SSA register reads and eliminates memory stores.

### Differentiators (Competitive Advantages for Phase 2)
- **FEAT-04: Redundant Load Elimination (RLE)**:
  - If a memory load is dominated by a prior store or load to the exact same place with no intervening clobbering `MemoryDef`, the load is eliminated and replaced by the cached SSA value.
- **FEAT-05: Dead Store Elimination (DSE)**:
  - Eliminates writes that are immediately overwritten or written to stack slots that are never subsequently read before function return.
- **FEAT-06: Seamless Integration with NumLang Supercompiler**:
  - Eliminating memory loads and stores exposes scalar loops to the algebraic supercompiler (`src/opt/supercompiler/`), enabling closed-form induction collapse on loops previously blocked by struct memory operations.

### Anti-Features (What We Deliberately Defer / Avoid)
- **Full Interprocedural Heap Escape Analysis**: Not needed yet in Phase 2; stack variables and struct values in NumLang are currently passed by value or value-lowered.
- **Dynamic C Pointer Casting / Unchecked Raw Pointers**: Phase 3 will introduce slices and C ABI; Phase 2 focuses strictly on stack locals, arrays, and structs.
