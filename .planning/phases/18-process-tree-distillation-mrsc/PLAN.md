# Phase 18: Global Process-Tree Distillation & MRSC Interface — Plan

> **Phase**: 18
> **Status**: Completed
> **Traceability**: Master Plan Part II, Requirements DIST-MRSC-PROTO-01..04
> **Milestone**: Core Supercompiler & Language Extensions (Phases 10–19)

## Objective
Implement the command-line flags and architectural interfaces for global process-tree transformation (Hamilton Distillation) and Multi-Result Supercompilation (MRSC).

## Requirements
- **DIST-MRSC-PROTO-01**: Define global process-tree data structures in `src/mir/supercompiler/distill.rs`.
- **DIST-MRSC-PROTO-02**: Scaffold multi-result configuration hypergraph generation in `src/mir/supercompiler/mrsc.rs`.
- **DIST-MRSC-PROTO-03**: Integrate CLI flags `--mode <classic|distill|mrsc>` and `--mrsc-objective <size|branch|pareto>`.
- **DIST-MRSC-PROTO-04**: Implement unit tests verifying command-line dispatch and baseline transformation passes.

## Key Deliverables
- `src/mir/supercompiler/distill.rs`, `src/mir/supercompiler/mrsc.rs`, `src/main.rs`

## Verification
- CLI dispatches accurately to distillation and MRSC submodules.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
