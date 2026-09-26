---
milestone: remediation-and-frontier
name: "Remediation & Frontier (Phases 20-28)"
status: in_progress
governance: "INTEGRITY_RULES.md"
---

# Project State

## Current Position

Phase: **Phase 27 — Honest High-Precision Benchmarks & Direct Supercompiler Comparisons** (NEXT)
Plan: `master_remediation_plan.md`
Status: Phase 26 committed (`55fe450`). All Lean 4 proofs pass `lake build` with zero errors, zero warnings, zero `sorry`, and zero `axiom`. Commencing Phase 27.
Last activity: 2026-09-26 — Phase 26 executed and verified.

## Progress

- [x] Phase 1–13: Production Compiler v1.0, Types, Backends, BCE, SROA
- [x] Phase 2.1–2.4: MemorySSA, Alias Analysis & Mem2Reg
- [x] Phase 10: Turchin Supercompiler Core, ADTs, 1st Futamura Projection
- [x] Phase 11: Higher-Order Functions, Lambdas, Closures, Indirect Calls
- [x] Phase 12: Generic Types `<T, U>`, Monomorphization Pass
- [x] Phase 13: Heap Memory `Box<T>`, Deref, Symbolic Heap
- [x] Phase 14–19: Advanced Prototyping (Fuzzing, Benchmarks, CLI, Docker)
- [x] **Milestone Audit**: Comprehensive independent review (`honest_review.md`)
- [x] **Phase 20: Fix Core Residualization, Knot Transfers & Textbook MSG** (COMMITTED `12c0867`)
- [x] **Phase 21: Real Hamilton Global Process-Tree Distillation with Deforestation** (COMMITTED `dc1b0ba`)
- [x] **Phase 22: Real Multi-Result Supercompilation (MRSC) Hypergraph Search and Pareto Selection** (COMMITTED `dea4cc3`)
- [x] **Phase 23: Real Polyhedral Loop & Stencil Deforestation with Buffer Contraction** (COMMITTED `5318b62`)
- [x] **Phase 24: Formal SMT-Based Translation Validation & Simulation Preorder** (COMMITTED `e7d3db1`)
- [x] **Phase 25: Genuine Self-Applicable Specializer MinSpec.nl for 2nd and 3rd Futamura Projections** (COMMITTED `81b2604`)
- [x] **Phase 26: Rigorous Lean 4 Verification — Zero Axioms, Recursive Semantics** (COMMITTED `55fe450`)
- [ ] **Phase 27: Honest High-Precision Benchmarks & Direct Supercompiler Comparisons** (NEXT)
- [ ] Phase 28: Paper Rewrite & Reproducibility Package

## Accumulated Context

### Critical Audit Findings & Decisions
- **Residualization Fix**: Fixed in Phase 20.
- **Textbook MSG**: Fixed in Phase 20.
- **Genuine Algorithms**: All rebuilt per INTEGRITY_RULES.md — Phases 21–24.
- **Futamura Self-Application**: Resolved in Phase 25 via `src/stdlib/minspec.nl`.
- **Lean 4 Proofs**: Resolved in Phase 26 — `axiom kruskal_tree_theorem` removed; `Semantics.lean` now models recursive function environments and heap; `Termination.lean` and `Driving.lean` carry constructive proofs. Zero `sorry`, zero `axiom`.
- **Safe box() Pattern**: Never write `box(Cons(..., box(...)))` inline when large enum variables are live. Always expand to explicit intermediate variables.
- **Linker Stack Patch**: `/STACK:16777216,1048576` in `src/codegen/linker.rs`.
- **Benchmarking Overhaul** (Phase 27 — CURRENT FOCUS):
  - `bench/harness/runner.py` `measure_execution_times` times `subprocess.run()` (process spawn ~14-18ms on Windows), not code execution. Must be replaced with in-process timing: N ≥ 10,000 iterations of the core computation inside the binary, measured with `QueryPerformanceCounter`/`clock_gettime`.
  - `bench/data/results.csv` is contaminated — `numlang_super` shows ~16× slower than baseline due to timing artifact.
  - No SPSC/HOSC comparison exists. Need KMP, double-nrev, Peano mul, power-spec benchmarks.
  - `bench/c/append3.c` double-free was already fixed.
