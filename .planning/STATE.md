---
milestone: remediation-and-frontier
name: "Remediation & Frontier (Phases 20-28)"
status: in_progress
governance: "INTEGRITY_RULES.md"
---

# Project State

## Current Position

Phase: **Phase 26 — Rigorous Lean 4 Verification (Zero Axioms, Recursive Semantics)** (NEXT)
Plan: `master_remediation_plan.md`
Status: Phase 25 committed (`81b2604`). All 3 Futamura projections verified sound via `verify_soundness`. `true_futamura_projections_tests` 4/4 green. Zero clippy warnings. Commencing Phase 26.
Last activity: 2026-09-26 — Phase 25 executed and verified.

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
- [ ] **Phase 26: Rigorous Lean 4 Verification (Zero Axioms, Recursive Semantics)** (NEXT)
- [ ] Phase 27: Honest High-Precision Benchmarks & Supercompiler Comparisons
- [ ] Phase 28: Paper Rewrite & Reproducibility Package

## Accumulated Context

### Critical Audit Findings & Decisions
- **Residualization Fix**: Fixed in Phase 20 — knot edges in `residualize.rs` now transfer variable state updates; Phi nodes correctly remapped.
- **Textbook MSG**: Fixed in Phase 20 — polynomial curve-fitter replaced with Plotkin/Sørensen anti-unification.
- **Genuine Algorithms Mandate**: All rebuilt per INTEGRITY_RULES.md — `distill.rs` (Phase 21), `mrsc.rs` (Phase 22), `polyhedral.rs` (Phase 23), `validate.rs` (Phase 24).
- **Futamura Self-Application**: Resolved in Phase 25 — `src/stdlib/minspec.nl` is a self-applicable partial evaluator written in NumLang. `verify_soundness(prog_id, input_x)` proves all 3 projections yield identical residual programs and identical outputs for programs 1, 2, 3.
- **Safe box() Pattern (Phase 25 lesson)**: Never write `box(Cons(..., box(...)))` inline when large enum variables are live on stack. Always expand to explicit intermediate variables to avoid Windows ACCESS_VIOLATION in Cranelift codegen.
- **Linker Stack Patch**: `/STACK:16777216,1048576` in `src/codegen/linker.rs` — raises Windows stack to 16MB for deep recursive enum functions.
- **Lean 4 Proof Integrity** (Phase 26): `axiom kruskal_tree_theorem` must be removed; proofs must constructively verify termination and semantic preservation over recursive semantics with heap and recursive function environments. `lake build` must pass with zero errors, zero warnings, zero `sorry`, zero `axiom`.
- **Benchmarking Overhaul** (Phase 27): `runner.py`'s `subprocess.run()` timing measures Windows process spawn (~14-18ms). Must be replaced with in-process microsecond hardware performance counter timing across >= 10,000 iterations.
