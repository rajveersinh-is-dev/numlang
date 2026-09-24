# Architecture Research: MemorySSA & Alias Analysis

**Domain:** Middle-End SSA Optimizer Architecture
**Researched:** 2026-09-24
**Confidence:** HIGH

## Component Architecture

```
                       TYPED AST
                           │
                           ▼
                  src/mir/lower.rs
             (TypedAST -> Raw MIR CFG)
                           │
                           ▼
                  src/mir/dominance.rs
             (Dominator Tree & Dominance Frontiers)
                           │
                           ▼
                  src/mir/alias.rs
             (Flow-Sensitive Points-To & Alias Analysis)
                           │
                           ▼
                  src/mir/memory_ssa.rs
             (MemoryDef, MemoryUse, MemoryPhi Graph)
                           │
                           ▼
                  src/mir/mem2reg.rs
             (Mem2Reg SSA Promotion Pass + DSE + RLE)
                           │
                           ▼
                  OPTIMIZED MIR CFG
                  (Pure SSA Registers)
                           │
                           ▼
                 Backend / Supercompiler
```

## Module Responsibilities

1. **`src/mir/alias.rs`**:
   - `enum AliasResult { NoAlias, MustAlias, MayAlias }`
   - `struct AliasAnalysis`: Computes base place roots (`Place.local`), projection paths (`Field`, `Index`), and determines disjointness.
   - Provides `query_alias(place_a: &Place, place_b: &Place) -> AliasResult`.

2. **`src/mir/memory_ssa.rs`**:
   - `struct MemoryVersionId(usize)`
   - `enum MemoryAccess { Def { id: MemoryVersionId, place: Place }, Use { id: MemoryVersionId, place: Place } }`
   - `struct MemoryPhi { id: MemoryVersionId, inputs: Vec<(BasicBlockId, MemoryVersionId)> }`
   - `struct MemorySsa`: Annotates each statement in `MirBasicBlock` with its memory effects and tracks reaching memory definitions.

3. **`src/mir/mem2reg.rs`**:
   - Analyzes `MirFunction.locals` to collect candidate non-aliased locals.
   - Computes iterated dominance frontiers (`IDF`) using `DominanceInfo.dominance_frontiers` from `dominance.rs`.
   - Places $\phi$-nodes (basic block arguments or SSA phi instructions) at join points.
   - Traverses the dominator tree (`idom`), maintaining a variable value stack to rewrite loads into register uses.

4. **Integration with `src/mir/mod.rs` & `src/main.rs`**:
   - Extend `MirFunction` or provide transformation pipeline `optimize_mir(program: &mut MirProgram)`.
   - Expose `--emit-memory-ssa` or include in `--emit-mir` output.
