# NumLang Compiler

## What This Is

NumLang is a high-performance mathematical systems programming language compiler targeting native Windows x86-64 with cross-platform build support. It combines Cranelift AOT code generation with an automatic algebraic supercompiler capable of deriving closed-form mathematical equations and accelerating coupled recurrences from $O(N) \to O(1)$ in $< 100\text{ ns}$.

## Core Value

Achieve bare-metal performance and closed-form algorithmic collapse without compromises: zero benchmark cheats, strict algebraic correctness, and maximal register promotion.

## Current Milestone: v2.0 Phase 2: MemorySSA & Alias Analysis

**Goal:** Implement MemorySSA, field-sensitive alias analysis, and an aggressive Mem2Reg register promotion engine over NumLang's MIR, unlocking maximum optimization and supercompilation factors.

**Target features:**
- First-class `MemorySSA` (`MemoryDef`, `MemoryUse`, `MemoryPhi`).
- Flow-sensitive and field-sensitive Points-To & Alias Analysis (`src/mir/alias.rs`).
- Iterated dominance frontier (IDF) `Mem2Reg` pass promoting stack variables and struct fields into pure SSA registers.
- Redundant Load Elimination (RLE) and Dead Store Elimination (DSE).
- Full CLI visualization (`--emit-memory-ssa`) and comprehensive test suite (`tests/memory_ssa_tests.rs`).

## Requirements

### Validated
- [x] v1.0 Production Finishing Plan: 13 phases completed, 36 test suites green, zero Clippy warnings, 100% benchmark parity.
- [x] v2.0 Phase 1: High-Level Mid-Level Intermediate Representation (MIR) SSA CFG, basic block terminators, dominance analysis, and TypedAST lowering.

### Active
- [ ] MemorySSA representation and graph construction (MSSA-01, MSSA-02, MSSA-03).
- [ ] Field-sensitive alias analysis for places and struct projections (ALIAS-01..04).
- [ ] Mem2Reg register promotion and memory optimization passes (M2R-01..04).
- [ ] CLI flags and comprehensive verification suite (TEST-01..03).

### Out of Scope
- Whole-program interprocedural heap alias analysis (deferred to Phase 3/4).
- Raw unchecked pointer casting.

## Context
- Compiler pipeline: Lexer (`logos`) -> Parser (`pratt`) -> Typechecker -> Optimizer -> MIR (`src/mir/`) -> Cranelift AOT -> Linker.
- Prior milestone v2.0 Phase 1 landed `src/mir/mod.rs`, `src/mir/dominance.rs`, and `src/mir/lower.rs`.

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state
