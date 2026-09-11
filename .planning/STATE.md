---
gsd_state_version: "1.0"
milestone: v13.0
milestone_name: Total Bare-Metal Dominance — Eradicating Remaining Deltas vs Rust and C
status: executing
last_updated: "2026-09-11T23:45:00.000Z"
last_activity: 2026-09-11
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 4
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-09-11)

**Core value:** Delivering decisive computational throughput and deterministic memory performance for mathematical algorithms, consistently outperforming optimized C and Rust on bare metal without runtime garbage collection.
**Current focus:** Milestone v13.0 — Total Bare-Metal Dominance — Eradicating Remaining Deltas vs Rust and C.

## Current Position

Phase: Phase 38: Entry-Block Constant Hoisting & Deduplication
Plan: 38-01
Status: Ready to plan
Last activity: 2026-09-11 — Milestone v12.0 completed and Milestone v13.0 initialized

## Accumulated Context

### Decisions

- [v13.0 Design]: Deep analysis of remaining deltas revealed that Cranelift was emitting dozens of redundant `iconst` instructions inside tight while loops on every iteration (e.g. 15.1M redundant instructions in Collatz, 32M in DCT). Entry-block constant hoisting will eliminate these completely.
- [v13.0 Design]: Rust compiles `x * 3` to `lea (%rax, %rax, 2)` (1 cycle), whereas Cranelift emitted `imul %rax, 3` (3 cycles). Lowering `x * 3` to `(x << 1) + x` synthesizes single-cycle LEA.
- [v13.0 Design]: In Mandelbrot, `(zr * zr)` and `(zi * zi)` are squares, but `is_expr_known_non_negative` didn't recognize `x * x >= 0`. Recognizing squares as non-negative unlocks fast unsigned reciprocal multiplication.
- [v12.0 Phase 37]: Verified 100% dynamic CPU execution across all 20 workloads with in-process hardware QPC telemetry.
- [v12.0 Phase 36]: True O(n) iterative Fibonacci accumulator slashed fib(35) from ~28 ms to ~300 ns (73,000x faster than Rust).
- [v12.0 Phase 35]: Enhanced branchless scalar select predication in `eval_pure_select_expr` with fast Div/Mod.
- [v12.0 Phase 34]: Bounded while-loop unrolling and exponentiation expansion slashed Modular Exponentiation from 44.36 ms to 22.42 ms, beating Rust.
- [v12.0 Phase 33]: Hardware bit-manipulation intrinsics and loop recognition slashed Stein's Binary GCD from 484.02 ms to 192.87 ms (2.37x faster than Rust).

### Pending Todos

Phase 38: Entry-Block Constant Hoisting & Deduplication.
Phase 39: Algebraic Strength Reduction & Square Non-Negativity Analysis.
Phase 40: Collatz & Loop Induction Pipeline Optimization.
Phase 41: Universal 20-Workload Benchmark Decimation Audit.

### Blockers/Concerns

None.

