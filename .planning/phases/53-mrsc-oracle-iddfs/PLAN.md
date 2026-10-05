# Phase 53: Exhaustive MRSC Oracle with IDDFS & Pareto Cost Model — Plan

> **Phase**: 53
> **Status**: Planned (Awaiting User Signal to Execute)
> **Traceability**: Master Plan §5, Requirements ORACLE-01..05
> **Closes gap vs**: MRSC research prototype (Mitchell & Klyuchnikov)

## Objective
Implement an unbounded iterative deepening DFS oracle over the MRSC configuration hypergraph (`--mrsc-exhaustive`), a 4-dimensional `MrscCostModel`, and Pareto frontier extraction to find globally optimal residual programs that bounded online MRSC misses.

## Root Cause of Loss
NumLang's MRSC uses heuristic bounded whistle firing (depth <= 8-15). Mitchell's prototype uses unbounded BFS and discovers optimal residuals requiring >15 driving steps. NumLang's whistle forces premature generalization before the optimal specialization is reached.

## Requirements
- **ORACLE-01**: Implement `src/mir/supercompiler/mrsc_oracle.rs`: IDDFS over the MRSC configuration hypergraph with configurable depth and `--mrsc-exhaustive` CLI flag.
- **ORACLE-02**: Implement `MrscCostModel` in `src/mir/supercompiler/mrsc.rs`: 4 dimensions (dynamic step count, allocation count, residual block count, register pressure).
- **ORACLE-03**: Implement Pareto frontier extraction: P dominates Q iff cost(P) <= cost(Q) componentwise. Select winner via `--mrsc-objective speed|size|balanced`.
- **ORACLE-04**: Integrate IDDFS-winning residuals into the L2 disk specialization cache keyed by structural hash.
- **ORACLE-05**: Verify in `tests/mrsc_oracle_tests.rs` that IDDFS depth >= 20 finds strictly smaller residuals than bounded online MRSC.

## Verification
- `cargo test --test mrsc_oracle_tests` passes 100%.
- IDDFS discovers strictly smaller (by step count) residuals on >= 3 benchmark programs.
- Pareto selection is deterministic across repeated runs.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
