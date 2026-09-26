---
milestone: remediation-and-frontier
name: "Remediation & Frontier (Phases 20-28)"
status: in_progress
governance: "INTEGRITY_RULES.md"
---

# Project State

## Current Position

Phase: **Phase 25 — Genuine Self-Applicable Specializer MinSpec.nl for 2nd and 3rd Futamura Projections** (IN PROGRESS)
Plan: `master_remediation_plan.md`
Status: Phase 24 committed; Phase 25 is partially implemented. `src/stdlib/minspec.nl` core functions work (`spec_eval`, `min_spec`, `first_futamura`, `expr_eq`). `verify_soundness` is stubbed — crashes when two large SpecExpr values + nested box() constructors are simultaneously live. Currently debugging via bisection.
Last activity: 2026-09-26 — bisection of verify_soundness crash; safe explicit-variable pattern confirmed.

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
- [ ] **Phase 25: Genuine Self-Applicable Specializer MinSpec.nl** (IN PROGRESS — crash fix + tests + commit remaining)
- [ ] Phase 26: Rigorous Lean 4 Verification (Zero Axioms, Recursive Semantics)
- [ ] Phase 27: Honest High-Precision Benchmarks & Supercompiler Comparisons
- [ ] Phase 28: Paper Rewrite & Reproducibility Package

## Accumulated Context

### Critical Audit Findings & Decisions
- **Residualization Fix**: `residualize.rs` knot edges (`ProcessEdge::Knot`) failed to transfer variable state updates, and Phi nodes retained unmapped `BasicBlockId`s, causing heap binaries (`nrev`, `append3`, `tree_flip`, `peano_mul`) to crash. Fixed in Phase 20.
- **Textbook MSG**: Replaced polynomial curve-fitter heuristic with Plotkin/Sørensen anti-unification. Fixed in Phase 20.
- **Genuine Algorithms Mandate**: `distill.rs` (Phase 21), `mrsc.rs` (Phase 22), `polyhedral.rs` (Phase 23), and `validate.rs` (Phase 24) rebuilt as genuine implementations per INTEGRITY_RULES.md.
- **Futamura Self-Application**: The 2nd and 3rd Futamura projections require `MinSpec.nl` written in NumLang itself. In progress in Phase 25 — `src/stdlib/minspec.nl` implements the self-applicable specializer. Crash in `verify_soundness` being debugged (stack/codegen issue with nested box() constructors while large SpecExpr is live).
- **Safe box() Pattern**: Never write `box(Cons(..., box(...)))` inline when large enum variables are live. Always expand to explicit intermediate variables (`let inner = ...; let b = box(inner); let outer = Cons(..., b);`).
- **Linker Stack Patch**: `/STACK:16777216,1048576` added to `src/codegen/linker.rs` to raise Windows stack to 16MB for deep recursive enum functions.
- **Lean 4 Proof Integrity**: `axiom kruskal_tree_theorem` must be removed; proofs must constructively verify termination and semantic preservation over recursive semantics. (Phase 26)
- **Benchmarking Overhaul**: `runner.py`'s `subprocess.run()` timing measures Windows process spawn (~14-18ms) rather than code execution. Must be replaced with in-process microsecond hardware performance counter timing across >= 10,000 iterations. (Phase 27)
