---
milestone: remediation-and-frontier
name: "Remediation & Frontier (Phases 20-28)"
status: in_progress
governance: "INTEGRITY_RULES.md"
---

# Project State

## Current Position

Phase: **Phase 28 — Paper Rewrite & 1-Click Reproducible Artifact Package** (NEXT — FINAL PHASE)
Plan: `master_remediation_plan.md`
Status: Phase 27 committed (`fd030fa`). In-process benchmark timing fixed; 5 canonical literature benchmarks added (KMP, double-nrev, power-spec, Peano mul, interpreter-spec); SPSC/HOSC reference baselines in `bench/data/reference_baselines.csv`; `bench/data/results.csv` regenerated with honest per-iteration microsecond data; `tests/benchmark_correctness_tests.rs` 5/5 green. One known CRASH: `append3,c_opt` exits code 3221226356 — recorded transparently in results.csv.
Last activity: 2026-09-26 — Phase 27 executed and verified.

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
- [x] **Phase 21: Real Hamilton Global Process-Tree Distillation** (COMMITTED `dc1b0ba`)
- [x] **Phase 22: Real MRSC Hypergraph Search and Pareto Selection** (COMMITTED `dea4cc3`)
- [x] **Phase 23: Real Polyhedral Loop & Stencil Deforestation** (COMMITTED `5318b62`)
- [x] **Phase 24: Formal SMT-Based Translation Validation** (COMMITTED `e7d3db1`)
- [x] **Phase 25: Genuine Self-Applicable Specializer MinSpec.nl** (COMMITTED `81b2604`)
- [x] **Phase 26: Rigorous Lean 4 Verification — Zero Axioms, Recursive Semantics** (COMMITTED `55fe450`)
- [x] **Phase 27: Honest High-Precision Benchmarks & Direct Supercompiler Comparisons** (COMMITTED `fd030fa`)
- [ ] **Phase 28: Paper Rewrite & 1-Click Reproducible Artifact Package** (NEXT — FINAL)

## Accumulated Context

### Critical Audit Findings & Decisions
- All algorithmic stubs resolved: Phases 21–24.
- Futamura projections: Resolved in Phase 25 via `src/stdlib/minspec.nl`.
- Lean 4 proofs: Resolved in Phase 26 — zero `axiom`, zero `sorry`, recursive semantics modeled.
- Benchmark timing: Fixed in Phase 27 — in-process per-iteration measurement, 30 rounds, 95% CI.
- **Paper integrity** (Phase 28 — CURRENT FOCUS):
  - `paper/main.tex` contains hardcoded claims e.g. "12,000--85,000 lines/second" throughput not backed by verified pipeline.
  - `\begin{tabular}...\toprule\end{tabular}` is empty — Table 1 benchmark data must be auto-generated from `bench/data/results.csv`.
  - `bench/harness/generate_tables.py` does NOT exist yet — must be created.
  - `docker/Dockerfile` has `RUN lake build || true` — Lean build failure is silently swallowed; must be `RUN lake build` (no `|| true`).
  - `docker/Dockerfile` CMD references `scripts/run_all_experiments.sh` — must verify this script exists and runs cleanly.
  - Sections 4 (Futamura), 5 (Lean 4 proofs), and 6 (Evaluation) need rewriting with verified data.
