# Stack Research: MemorySSA & Alias Analysis

**Domain:** Optimizing Systems Compiler (Middle-End Intermediate Representation)
**Researched:** 2026-09-24
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology / Component | Version / Target | Purpose | Why Recommended |
|---|---|---|---|
| **Rust 2021 Edition** | 1.80+ | Core compiler implementation language | Zero-cost abstractions, memory safety, robust standard data structures (`BTreeSet`, `HashMap`, `Vec`). |
| **Typed MIR CFG** | Internal (v2.0 Phase 1) | SSA-form representation of NumLang functions | Phase 1 established `MirProgram`, `BasicBlock`, and `Place` projections needed for alias analysis. |
| **MemorySSA Token Engine** | Internal | Models memory state versions via explicit tokens | Standard LLVM/GCC industry architecture for scalable, linear-time memory dependency tracking. |
| **Cranelift Backend** | Cranelift 0.110+ | Native x86-64 code generation | High-velocity AOT compilation and machine lowering. |

### Supporting Algorithms & Patterns

| Technique | Origin / Reference | Purpose | When to Use |
|---|---|---|---|
| **Cytron et al. SSA Construction** | Cytron et al. (TOPLAS 1991) | Mem2Reg promotion via iterated dominance frontiers (IDF) | Promoting all non-escaping stack variables into pure SSA registers. |
| **Andersen-style Points-To Analysis** | Andersen (1994) / Steensgaard | Flow-sensitive subset constraint alias analysis | Distinguishing disjoint allocations, stack places, and struct field projections. |
| **MemorySSA Graph** | Berlin & Burgess (LLVM), Novillo (GCC) | Versioned `MemoryDef`, `MemoryUse`, and `MemoryPhi` | Powering Redundant Load Elimination (RLE) and Dead Store Elimination (DSE) without quadratic pair-wise alias checks. |

## Alternatives Considered

| Recommended | Alternative | Trade-off / Decision |
|---|---|---|
| **MemorySSA** | Direct N-squared Pairwise Alias Checks | Pairwise query checks scale as $O(N^2)$ inside loops, stalling compilation. MemorySSA maintains explicit def-use links for linear $O(N)$ traversal. |
| **Cytron Mem2Reg via Dominance Frontiers** | Basic Block Argument Threading | Cytron algorithm uses the already computed `dominance_frontiers` from `src/mir/dominance.rs` to place $\phi$-nodes minimally. |
| **Field-Sensitive Stack Points-To** | Steensgaard (Equivalence-based) | Steensgaard collapses fields and aliased sets into coarse equivalence classes, creating false dependencies. Field sensitivity preserves independent scalar registers for struct fields. |

## What NOT to Use
- **Do not introduce heavy external C/C++ dependencies (e.g. LLVM C API)**: Keeps NumLang self-contained, lightweight, and fast to build with pure Rust `cargo`.
- **Do not use naive whole-program pointer unification**: Would treat all variables as potentially aliased, disabling scalar promotion.
