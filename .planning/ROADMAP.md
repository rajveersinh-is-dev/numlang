# Roadmap: NumLang (v2.0 Phase 2: MemorySSA & Alias Analysis)

## Overview

Elevating NumLang's MIR with an industrial-grade MemorySSA, field-sensitive alias analysis, and an aggressive Mem2Reg promotion pass to eradicate memory bottlenecks and unlock maximum compiler optimization.

## Phases

### Phase 2.1: MemorySSA Core & Graph Construction
- **Goal**: Model memory states as versioned SSA tokens attached to MIR statements.
- **Scope**:
  - `src/mir/memory_ssa.rs`: `MemoryVersionId`, `MemoryAccess` (`MemoryDef`, `MemoryUse`), `MemoryPhi`.
  - Traverse MIR blocks and build memory def-use chains.
  - Insert `MemoryPhi` nodes at basic block join points with multiple reaching memory versions.
- **Verification**: Unit tests verifying correct memory version propagation across branching blocks.

### Phase 2.2: Points-To & Field-Sensitive Alias Analysis
- **Goal**: Statically prove disjointness between independent variables and struct fields.
- **Scope**:
  - `src/mir/alias.rs`: `AliasAnalysis` engine with `NoAlias`, `MustAlias`, `MayAlias`.
  - Base place root analysis: distinct stack locals never alias.
  - Field-sensitive path analysis: `p.x` vs `p.y` provably `NoAlias`.
  - Array constant index analysis: `arr[0]` vs `arr[1]` provably `NoAlias`.
- **Verification**: Alias query tests validating precision and soundness.

### Phase 2.3: Mem2Reg Promotion Engine & Memory Optimizations
- **Goal**: Promote non-escaping memory stack allocations to pure SSA virtual registers.
- **Scope**:
  - `src/mir/mem2reg.rs`: Candidate identification for non-escaped locals and struct fields.
  - Iterated Dominance Frontier (IDF) computation via `src/mir/dominance.rs`.
  - $\phi$-placement and variable stack renaming pass.
  - Redundant Load Elimination (RLE) and Dead Store Elimination (DSE).
- **Verification**: Tests checking that memory loads and stores are removed, replaced by pure SSA register uses.

### Phase 2.4: CLI Tooling, Integration & Verification Gate
- **Goal**: Full compiler integration, developer visibility, and zero-defect quality gate.
- **Scope**:
  - Add `--emit-memory-ssa` to `src/main.rs` for visual inspection of memory tokens.
  - Create integration test suite `tests/memory_ssa_tests.rs`.
  - Verify 100% test suite pass rate (< 10s) across all 38 test suites.
  - Verify 0 Clippy warnings (`cargo clippy --all-targets -- -D warnings`).
- **Verification**: Clean release build, full test run, and regression benchmarks.
