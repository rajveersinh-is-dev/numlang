# Phase 66: Incremental Modular Supercompilation with Fine-Grained Invalidation — Plan

> **Phase**: 66
> **Status**: Complete
> **Traceability**: Master Plan Part VI, Requirements MODCACHE-01..05
> **Milestone**: World's Fastest General Supercompiler (Phases 59–66)

## Objective
Track an inter-function callee dependency graph for fine-grained specialization cache invalidation, enabling incremental supercompilation during multi-file builds and live edit-compile cycles.

## Root Cause / Motivation
Currently, any change to `program_funcs` forces a full re-supercompilation of all functions. In large production codebases with hundreds of functions, whole-program recompilation incurs latency prohibitive for interactive development. Maintaining an accurate dependency graph allows reusing cached specializations for functions whose bodies and transitive dependencies remain unmodified.

## Requirements
- **MODCACHE-01**: Extend `SpecializationCache` with an inter-function dependency graph: `HashMap<CacheKey, Vec<CacheKey>>` recording transitive callee dependencies.
- **MODCACHE-02**: Compute composite cache keys incorporating the SHA-256 hashes of the function MIR body and all reachable callee bodies.
- **MODCACHE-03**: Implement fine-grained cache invalidation: when a function changes, invalidate only its upstream callers in the dependency DAG.
- **MODCACHE-04**: Serialize dependency graph and disk cache entries to `.numlang_cache/deps.json` under the `--incremental` CLI flag.
- **MODCACHE-05**: Verification in `tests/incremental_cache_tests.rs`: in a multi-function module, modify a single leaf function and verify that only dependent callers are re-specialized, achieving $\ge 70\%$ cache reuse.

## Key Deliverables
- `src/mir/supercompiler/cache.rs`, `src/compiler.rs`, `src/main.rs`
- Test suite: `tests/incremental_cache_tests.rs`

## Verification Gate
- `cargo test --test incremental_cache_tests` passes 100%.
- Verified fine-grained invalidation triggers re-specialization only for affected functions.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
