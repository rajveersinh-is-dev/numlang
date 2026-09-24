---
milestone: v2.0-phase2
name: "MemorySSA & Alias Analysis"
status: planning
---

# Project State

## Current Position

Phase: Phase 2.2 (Points-To & Field-Sensitive Alias Analysis)
Plan: Ready to plan Phase 2.2
Status: Phase 2.1 complete, proceeding to Phase 2.2
Last activity: 2026-09-24 — Phase 2.1 MemorySSA Core completed

## Progress

- [x] Phase 2.1: MemorySSA Core & Graph Construction
- [ ] Phase 2.2: Points-To & Field-Sensitive Alias Analysis
- [ ] Phase 2.3: Mem2Reg Promotion Engine & Memory Optimizations
- [ ] Phase 2.4: CLI Tooling, Integration & Verification Gate

## Accumulated Context

### Key Decisions
- Adopted MemorySSA token architecture (`MemoryDef`, `MemoryUse`, `MemoryPhi`) for scalable linear-time memory dependency modeling.
- Enforcing field-sensitivity in alias analysis to allow individual struct fields (`p.x`, `p.y`) to promote independently into SSA registers.
- Cytron iterated dominance frontiers (`IDF`) over existing `src/mir/dominance.rs` dominator trees to guarantee minimal $\phi$-placement.
