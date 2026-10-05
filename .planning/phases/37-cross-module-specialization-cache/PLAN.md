# Phase 37: Cross-Module Specialization Cache — Plan

> **Phase**: 37
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 37
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Implement persistent two-level content-addressed SHA-256 specialization disk caching across compilation runs and modules.

## Root Cause / Motivation
Supercompilation is computationally intensive; identical function specializations across modules or repeated builds should be reused instantaneously.

## Requirements
- **CACHE-01**: Add `SpecializationCache`, `CacheKey`, and `CachedSpecialization` in `src/mir/supercompiler/cache.rs` with content-addressed 2-level fanout JSON disk caching.
- **CACHE-02**: Derive `serde` across MIR AST.
- **CACHE-03**: Wire cache lookups and stores into `supercompile_mir_program_with_cache`.
- **CACHE-04**: Expose `--cache-dir` and `--no-cache` CLI flags.

## Key Deliverables
- `src/mir/supercompiler/cache.rs`
- `tests/supercompiler_phase37_tests.rs`

## Verification
- `cargo test --test supercompiler_phase37_tests` passes 5/5 tests green.
- Second-run specializations achieve instant cache hits with exact binary equivalence.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
