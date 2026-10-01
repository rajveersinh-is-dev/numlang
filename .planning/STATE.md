---
gsd_state_version: "1.0"
milestone: hardening-and-soundness
current_phase: 41
current_phase_name: decouple-win32-and-true-posix-native-codegen
status: planned
last_updated: "2026-10-01T07:42:44.846Z"
last_activity: 2026-10-01
last_activity_desc: Phase 41 execution started
state_head: 77b0e7ab95099c7068bf062e3195be49d500a0c2
name: Hardening, Soundness & Architecture Remediation (Phases 41-45)
governance: INTEGRITY_RULES.md
origin: ADVERSARIAL_AUDIT.md
---

# Project State

## Current Position

Milestone: **Hardening, Soundness & Architecture Remediation (Phases 41–45)**  
Phase: 41 (decouple-win32-and-true-posix-native-codegen) — EXECUTING
Last activity: 2026-10-01 — Phase 41 execution started

## Milestone Progress

- [x] **Milestone 1**: Core Foundation & Language Extensions (Phases 1–19) [COMPLETED]
- [x] **Milestone 2**: Remediation, Frontier & Stabilization (Phases 20–40) [COMPLETED]
- [ ] **Milestone 3**: Hardening, Soundness & Architecture (Phases 41–45) [PLANNED]
  - [ ] **Phase 41**: Decouple Win32 & True POSIX Native Codegen (PORT-01..04)
  - [ ] **Phase 42**: Constructive Lean 4 Soundness Proofs (LEAN-04..07)
  - [ ] **Phase 43**: Authentic Self-Applicable Specializer & Futamura Projections (FUTA-05..08)
  - [ ] **Phase 44**: Scoped Arena Allocator & Memory Safety (MEM-01..04)
  - [ ] **Phase 45**: Architecture Decomposition, Codegen Unification & Hardening (ARCH-01..05)

## Accumulated Context

### Critical Audit Findings Addressed (from `ADVERSARIAL_AUDIT.md`)

1. **POSIX / Linux Portability**: Windows API symbols (`ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`) must be isolated behind `target_os = "windows"`; POSIX libc must be provided for Linux/macOS.
2. **Formal Verification Soundness**: Tautological constructors in Lean 4 (`SupercompilerProduces`, `FoldStep`, `NoopRemoval`) must be replaced with constructive function definitions, and `Step*` must be connected to `Evaluates`.
3. **Futamura Projections**: Identical function bodies in `minspec.nl` must be replaced with an authentic self-applicable partial evaluator.
4. **Memory Management**: Unbounded memory leaks from heap allocations without `free` will be eliminated via a scoped bump-arena runtime with loop-header resets.
5. **Architectural Hardening**: Decompose 9.3k LOC `cranelift_backend.rs`, retire duplicate AST supercompiler, eliminate raw `.unwrap()`s, and fix unchecked arithmetic in recurrence solving.

### Next Action

Run `/gsd-plan-phase 41` to generate the execution plan for **Phase 41: Decouple Win32 & True POSIX Native Codegen**.
