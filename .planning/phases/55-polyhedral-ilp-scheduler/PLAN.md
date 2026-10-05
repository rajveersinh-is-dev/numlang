# Phase 55: Pure-Rust Polyhedral ILP Scheduler (Pluto-style) — Plan

> **Phase**: 55
> **Status**: Planned (Awaiting User Signal to Execute)
> **Traceability**: Master Plan §7, Requirements ILP-01..05
> **Closes gap vs**: LLVM Polly / Pluto / ISL (Integer Set Library)

## Objective
Implement a full integer linear programming solver over affine iteration domains to compute Pluto-style legal multi-dimensional loop schedules, enabling loop tiling, loop skewing, and diamond tiling — capabilities beyond NumLang's current basic loop fusion.

## Root Cause of Loss
NumLang's `polyhedral.rs` extracts affine iteration domains and performs basic loop fusion but lacks an ILP solver. Polly/Pluto use ISL (an ILP solver over integer polyhedra) to compute optimal tiling schedules that maximize L2 cache reuse in nested matrix loops — a capability NumLang currently cannot match.

## Requirements
- **ILP-01**: Implement `src/mir/supercompiler/polyhedral_ilp.rs`: fraction-free Simplex (Bareiss algorithm, exact i128 arithmetic) over integer polyhedra to solve Farkas lemma dual LP for schedule feasibility.
- **ILP-02**: Implement Pluto permutability constraints: for each dependence (S_i, S_j, d), emit `theta_i . d >= 0` as LP row. Solve for schedule coefficients theta.
- **ILP-03**: Implement rectangular loop tiling in `polyhedral.rs`: given permutable schedule, apply T=32 rectangular tiling (outer tile loops + inner element loops).
- **ILP-04**: Emit tiled vectorized loop nests in residualized MIR; innermost tile dimension maps to vectorization metadata.
- **ILP-05**: Verify in `tests/polyhedral_ilp_tests.rs` that 3-nested matmul tiles legally and Simplex terminates in <= 1,000 pivots for <= 8 loop dimensions.

## Key Implementation Notes
- Bareiss pivot formula: `T'[i][j] = (T[p][j]*T[i][j] - T[i][p_col]*T[p][j]) / prev_pivot`. Integer division is exact by the Bareiss determinantal property.
- Default tile size T=32 targets 32 i64 elements = 256 bytes = single L1 cache line.
- After tiling, the inner element loop has no cross-tile dependences: LLVM auto-vectorizes into AVX2.

## Verification
- `cargo test --test polyhedral_ilp_tests` passes 100%.
- 3-nested `C[i][j] += A[i][k] * B[k][j]` produces tiled schedule legal under all dependences.
- Simplex terminates in <= 1,000 pivot steps for <= 8 loop dimensions.
- Intermediate array buffers contract to scalar temporaries after tiling.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
