# Phase 51: Lazy/Thunk SSA Extension & Codata Supercompilation — Plan

> **Phase**: 51
> **Status**: Planned (Awaiting User Signal to Execute)
> **Traceability**: Master Plan §3, Requirements LAZY-01..05
> **Closes gap vs**: GHC Supercompiler (Bolingbroke & Peyton Jones)

## Objective
Extend NumLang MIR with explicit thunk representation (`Rvalue::Thunk`, `Terminator::Force`), implement demand-propagation analysis, add a lazy symbolic driving mode in the supercompiler, and implement stream fusion to deforest infinite producer-consumer codata chains into zero-allocation single-pass loops — surpassing GHC on lazy functional pipeline workloads.

## Root Cause of Loss
NumLang's driving loop operates on strict call-by-value SSA semantics. When given an infinite lazy stream producer (e.g. `iterate f x`), the driving loop immediately attempts to unfold the producer without bound, triggering the homeomorphic embedding whistle and aborting. GHC's supercompiler is purpose-built for non-strict semantics and can drive through infinite thunk chains on demand.

## Requirements
- **LAZY-01**: Define `Rvalue::Thunk { body: MirBodyId, env: Vec<LocalId> }` and `Terminator::Force { thunk: LocalId, result: LocalId, cont: BasicBlockId }` in `src/mir/mod.rs`. Extend `MirPrinter` and validation pass.
- **LAZY-02**: Implement `src/mir/thunk_analysis.rs`: demand-propagation analysis computing which thunks are demanded on every execution path, enabling selective forcing at compile-time during driving.
- **LAZY-03**: Implement lazy driving mode in `src/mir/supercompiler/drive.rs`: represent unevaluated thunks as `SymTerm::Thunk(MirBodyId, Vec<SymTermId>)` and force only when downstream uses demand the value.
- **LAZY-04**: Implement stream fusion in `src/mir/supercompiler/distill.rs`: recognize producer-consumer thunk chains (`map`, `filter`, `take`, `zipWith`, `iterate`) and fuse into zero-allocation single-pass loops.
- **LAZY-05**: Verify in `tests/codata_supercompilation_tests.rs` that lazy streams fuse without divergence and produce zero heap allocation calls.

## Key Implementation Notes
- `SymTerm::Thunk` is a new variant in the symbolic state: does NOT expand unless forced by a downstream `Terminator::Force`.
- The thunk analysis distinguishes three demand levels: `Bottom` (never demanded), `GuardDemanded` (demanded in a branch condition), `FullyDemanded` (demanded on all paths).
- Stream fusion fires when `distill.rs` detects a bounded consumer (`take N`) driving a thunk-producer chain.
- All zero-allocation guarantees must be verified by inspecting LLVM IR for absence of `call @malloc` / `call @__nl_arena_alloc`.

## Verification
- `cargo test --test codata_supercompilation_tests` passes 100%.
- `take(1000, zipWith(+, iterate(*2, 1), iterate(*3, 1)))` compiles to a single counting loop with zero malloc calls in the emitted LLVM IR.
- GHC produces correct output from the same algorithm; NumLang output matches and NumLang wall time is competitive or faster.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
