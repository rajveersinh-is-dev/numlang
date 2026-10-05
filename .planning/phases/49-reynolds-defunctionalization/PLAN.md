# Phase 49: Deep Reynolds Defunctionalization & Higher-Order Deforestation — Plan

> **Phase**: 49  
> **Status**: Planned (Awaiting User Signal to Execute)  
> **Traceability**: Master Plan §4, Requirements DEFUN-01..05  

## Objective
Outperform HOSC and GHC on higher-order functional programs by compiling higher-order closures into zero-allocation, monomorphic, first-order SSA loops with static dispatch. Implement whole-program Reynolds defunctionalization, drive through concrete closure tags to prune branches, and deforest higher-order consumer-producer pipelines (`map-filter-fold`) into single-pass machine loops.

## Requirements
- **DEFUN-01**: Implement whole-program type-directed Reynolds defunctionalization in `src/mir/defunctionalize.rs`, generating discriminated union sum types `ClosureTag_<Sig>` for each closure call signature.
- **DEFUN-02**: Replace `Rvalue::ClosureAlloc` with typed tagged enum allocations carrying captured environment payloads.
- **DEFUN-03**: Lower `Terminator::IndirectCall` into direct `Terminator::SwitchInt` over tags, dispatching to monomorphic static `Terminator::Call` sites.
- **DEFUN-04**: Drive through closure tags in `src/mir/supercompiler/drive.rs` and deforest higher-order pipelines (`map`/`filter`/`fold`) in `src/mir/supercompiler/distill.rs` into single-pass, allocation-free loops.
- **DEFUN-05**: Eliminate intermediate closure objects via SROA and verify zero allocations and zero indirect calls in `tests/defunctionalize_deforestation_tests.rs`.

## Verification
- `cargo test --test defunctionalize_deforestation_tests` passes 100%.
- Verified zero allocations (`malloc` / `__nl_arena_alloc`) on higher-order stream fusion benchmarks.
- Measured runtime demonstrates $\ge 5\times$ speedup over GHC 9.4 `-O2` and $2\times$ over Rust iterator chains.
