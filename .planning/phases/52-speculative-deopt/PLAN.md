# Phase 52: Speculative Type Guards & Deoptimization Safepoints — Plan

> **Phase**: 52
> **Status**: Planned (Awaiting User Signal to Execute)
> **Traceability**: Master Plan §4, Requirements DEOPT-01..05
> **Closes gap vs**: GraalVM Truffle / V8 TurboFan

## Objective
Implement speculative type-specialized compilation with runtime deoptimization safety nets: `Terminator::TypeGuard` fast-path branches for polymorphic call sites, Cranelift deoptimization stubs reconstructing interpreter frames on type mismatch, and OSR entry point slots for Tier 0 -> Tier 1 hot-path upgrade.

## Root Cause of Loss
NumLang commits to a single static specialization at compile time. If the inferred type is incorrect at runtime, there is no fallback path. GraalVM Truffle speculatively compiles assuming a type profile and deoptimizes gracefully when violated.

## Requirements
- **DEOPT-01**: Implement `src/mir/speculate.rs`: type-profile analysis recording `TypeProfile { local, observed_tag, confidence }` at each polymorphic call site during symbolic driving.
- **DEOPT-02**: Emit `Terminator::TypeGuard { local, expected_tag, fast_path, deopt_stub }` in residualized MIR at call sites where `confidence >= 0.95`.
- **DEOPT-03**: Implement deoptimization stubs in `src/codegen/cranelift/deopt.rs`: `DeoptMetadata` table + runtime `__nl_deopt(deopt_id, ...)` reconstructing interpreter call frame from register values.
- **DEOPT-04**: Implement OSR entry points in Cranelift preambles: check `AtomicPtr<u8>` flag; jump to T1 code when non-null.
- **DEOPT-05**: Verify in `tests/speculative_deopt_tests.rs` that 9,999/10,000 i64 calls take the fast path and the 1 f64 call correctly deoptimizes with bit-identical output.

## Verification
- `cargo test --test speculative_deopt_tests` passes 100%.
- Fast-path execution rate >= 99% on uniform-type input.
- Deoptimized execution produces bit-identical output to unspecialized baseline.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
