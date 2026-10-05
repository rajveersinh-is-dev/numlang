# Phase 61: Fast Hash-Cons Whistle: O(1) Structural Identity — Plan

> **Phase**: 61
> **Status**: Planned
> **Traceability**: Master Plan Part VI, Requirements HASHCONS-01..05
> **Milestone**: World's Fastest General Supercompiler (Phases 59–66)

## Objective
Establish canonical hash-consing term interning, DAG depth and size caches, and structural hash comparison to achieve $O(1)$ structural identity checks and fast $O(k \log k)$ homeomorphic embedding checks during driving.

## Root Cause / Motivation
The homeomorphic embedding whistle (`state_embeds`) is called every time a loop header or function node is revisited. For programs with large symbolic states (many variables, deep term trees), this can consume $O(V \cdot D^2)$ time where $V$ is the number of state variables and $D$ is the term depth. By enforcing canonical hash-consing with algebraic normalization (Phase 59), structural identity is guaranteed to be $O(1)$ by pointer/ID equality, and precomputed sizes/depths allow pruning embedding subchecks immediately.

## Requirements
- **HASHCONS-01**: Enforce that structurally identical terms have the same `SymTermId` as an invariant of `TermInterner`.
- **HASHCONS-02**: Add a DAG depth cache (`depths: Vec<usize>`) and precomputed node size cache (`sizes: Vec<usize>`) to `TermInterner` so that embedding size filters operate in $O(1)$ time.
- **HASHCONS-03**: Compute structural FxHash for every `SymTermId` slot (`hashes: Vec<u64>`), allowing $O(1)$ equality tests before full structural tree comparison.
- **HASHCONS-04**: Optimize `is_embedded` and `state_embeds` in `src/mir/supercompiler/whistle.rs` using size and depth lower-bounds to skip non-embedding candidates.
- **HASHCONS-05**: Verification in `tests/fast_whistle_tests.rs`: benchmark whistle performance on programs with 50+ symbolic variables; achieve $\ge 5\times$ throughput speedup on large synthetic terms.

## Key Deliverables
- `src/mir/supercompiler/term.rs`, `src/mir/supercompiler/whistle.rs`
- Test suite: `tests/fast_whistle_tests.rs`

## Verification Gate
- `cargo test --test fast_whistle_tests` passes 100%.
- Benchmark demonstrates substantial speedup on deep symbolic trees.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
