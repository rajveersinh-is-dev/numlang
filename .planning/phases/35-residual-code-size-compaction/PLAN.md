# Phase 35: Optimal Residual Code Size (Post-Distillation Compaction) — Plan

> **Phase**: 35
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 35
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Implement pre-residualization process-tree compaction (`compact_process_tree`) and post-residualization MIR peephole compaction (`compact_mir_function`) with dead overflow node elimination and copy propagation.

## Root Cause / Motivation
Supercompilation and distillation can duplicate basic blocks or emit identity assignments, expanding residual binary code size without execution benefit.

## Requirements
- **COMPACT-01**: Implement pre-residualization process-tree compaction (`compact_process_tree`) with dead overflow node elimination and alpha-equivalent knot deduplication.
- **COMPACT-02**: Implement post-residualization MIR peephole pass (`compact_mir_function`) with identity assignment removal and conservative copy propagation (eta-reduction).
- **COMPACT-03**: Add `residual_block_count` and `residual_stmt_count` to `SupercompilerStats` and `--supercompile-stats` CLI.

## Key Deliverables
- `src/mir/supercompiler/compaction.rs`
- `src/mir/supercompiler/stats.rs`
- `tests/supercompiler_phase35_tests.rs`

## Verification
- `cargo test --test supercompiler_phase35_tests` passes 5/5 tests green.
- Verified reduction in residual block and statement counts without altering program semantics.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
