---
milestone: remediation-and-frontier
name: "Remediation & Frontier (Phases 20-28)"
status: in_progress
governance: "INTEGRITY_RULES.md"
---

# Project State

## Current Position

Phase: Transitioning into Phase 21 (Real Hamilton Global Process-Tree Distillation)
Plan: `master_remediation_plan.md` & `.planning/phases/21-global-distillation/PLAN.md`
Status: Phase 20 completed (100% green tests, 0 crashes on heap/recursive benchmarks, textbook MSG, parallel knot state transfers); commencing Phase 21.
Last activity: 2026-09-26 — Phase 20 executed and verified.

## Progress

- [x] Phase 1–13: Production Compiler v1.0, Types, Backends, BCE, SROA
- [x] Phase 2.1–2.4: MemorySSA, Alias Analysis & Mem2Reg
- [x] Phase 10: Turchin Supercompiler Core, ADTs, 1st Futamura Projection
- [x] Phase 11: Higher-Order Functions, Lambdas, Closures, Indirect Calls
- [x] Phase 12: Generic Types `<T, U>`, Monomorphization Pass
- [x] Phase 13: Heap Memory `Box<T>`, Deref, Symbolic Heap
- [x] Phase 14–19: Advanced Prototyping (Fuzzing, Benchmarks, CLI, Docker)
- [x] **Milestone Audit**: Comprehensive independent review (`honest_review.md`)
- [x] **Phase 20: Fix Core Residualization, Knot Transfers & Textbook MSG** (COMPLETED)
- [ ] **Phase 21: Real Hamilton Global Distillation** (NEXT)
- [ ] Phase 22: Real Mitchell & Klyuchnikov MRSC
- [ ] Phase 23: Real Polyhedral Loop & Stencil Deforestation
- [ ] Phase 24: Formal SMT-Based Translation Validation
- [ ] Phase 25: Genuine Self-Applicable Specializer & 2nd and 3rd Futamura Projections
- [ ] Phase 26: Rigorous Lean 4 Verification (Zero Axioms, Recursive Semantics)
- [ ] Phase 27: Honest High-Precision Benchmarks & Supercompiler Comparisons
- [ ] Phase 28: Paper Rewrite & Reproducibility Package

## Accumulated Context

### Critical Audit Findings & Decisions
- **Residualization Fix**: `residualize.rs` knot edges (`ProcessEdge::Knot`) failed to transfer variable state updates, and Phi nodes retained unmapped `BasicBlockId`s, causing heap binaries (`nrev`, `append3`, `tree_flip`, `peano_mul`) to crash. Fixing this is Phase 20's primary focus.
- **Textbook MSG**: Replace polynomial curve-fitter heuristic with Plotkin/Sørensen anti-unification.
- **Genuine Algorithms Mandate**: `distill.rs`, `mrsc.rs`, `polyhedral.rs`, and `validate.rs` are confirmed stubs and must be rebuilt as genuine implementations according to their original literature definitions.
- **Futamura Self-Application**: The 2nd and 3rd Futamura projections cannot be performed with a Rust specializer on NumLang code. `MinSpec.nl` must be implemented in NumLang source code.
- **Lean 4 Proof Integrity**: `axiom kruskal_tree_theorem` must be removed; proofs must constructively verify termination and semantic preservation over recursive semantics.
- **Benchmarking Overhaul**: `runner.py`'s `subprocess.run()` timing measures Windows process spawn (~14-18ms) rather than code execution. Must be replaced with in-process microsecond hardware performance counter timing across $\ge 10,000$ iterations.
