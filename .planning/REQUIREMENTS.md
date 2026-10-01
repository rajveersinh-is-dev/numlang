# Requirements: NumLang Hardening, Soundness & Architecture (Phases 41–45)

**Defined:** 2026-10-01  
**Governing Standard:** [`INTEGRITY_RULES.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md)  
**Origin Audit:** [`ADVERSARIAL_AUDIT.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/ADVERSARIAL_AUDIT.md)  
**Core Value:** Mathematically sound, independently reproducible, memory-safe, world-class systems supercompilation without tautological shortcuts, platform lock-in, or unbounded memory leakage.

---

## Hardening & Soundness Requirements

### 1. Decouple Win32 & True POSIX Native Codegen (Phase 41)
- [ ] **PORT-01**: Abstract runtime platform imports across native code generators (`src/codegen/cranelift_backend.rs`, `src/codegen/llvm_backend.rs`): target Windows API on `target_os = "windows"` (`ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`) and standard POSIX libc on `not(target_os = "windows")` (`exit`, `write(1, ...)`, `malloc`/`mmap`).
- [ ] **PORT-02**: Update `src/codegen/entry_bench.c` to use standard ISO C `exit((int)ret);` rather than Win32 `ExitProcess`.
- [ ] **PORT-03**: Fix `link_unix` in `src/codegen/linker.rs` and verify clean linking and execution of test programs and microbenchmarks on Linux.
- [ ] **PORT-04**: Validate that the Dockerfile (`docker/Dockerfile`) build and `docker/entrypoint.sh` complete end-to-end without unresolved Win32 symbols.

### 2. Constructive Lean 4 Soundness Proofs (Phase 42)
- [ ] **LEAN-04**: Connect small-step transition `Step` to `Evaluates` in `lean/Supercompiler/Semantics.lean`: Define `Evaluates fn env res` as existence of a terminating execution trace $\exists s_f, \text{Step}^* \langle 0, env \rangle s_f \land s_f.term = \text{Return}(\text{res})$.
- [ ] **LEAN-05**: Implement constructive optimization transforms as Lean definitions: dead-node elimination (`eliminate_dead_nodes`), no-op removal (`remove_nops`), and refinement branch pruning (`prune_unreachable_branches`).
- [ ] **LEAN-06**: Eliminate the tautological constructor definitions (`SupercompilerProduces.pipeline`, `FoldStep.fold`, `NoopRemoval.remove_nop`, `EtaReduction.copy_prop`) that assume `SemanticEquivalent` as an input premise.
- [ ] **LEAN-07**: Prove constructive semantic preservation theorems: for each constructive pass $T$, prove $\forall \text{fn}, \text{SemanticEquivalent} \; \text{fn} \; (T(\text{fn}))$, verifying under `lake build` with zero `sorry` and zero axioms.

### 3. Authentic Self-Applicable Specializer & Futamura Projections (Phase 43)
- [ ] **FUTA-05**: Rewrite `src/stdlib/minspec.nl` so that `min_spec(prog: SpecExpr, env: SpecEnv) -> SpecExpr` is a genuine, non-trivial partial evaluator capable of evaluating static expressions and preserving dynamic variables.
- [ ] **FUTA-06**: Implement authentic 1st Futamura projection: specialize an interpreter with respect to a static program expression, eliminating all interpreter interpretation dispatch.
- [ ] **FUTA-07**: Implement authentic 2nd Futamura projection: specialize `min_spec` with respect to an interpreter, producing a standalone compiled representation without hardcoded copy-paste shortcuts.
- [ ] **FUTA-08**: Implement authentic 3rd Futamura projection: specialize `min_spec` with respect to `min_spec`, yielding a compiler generator (`cogen`), and verify that $\text{cogen}(\text{interp})$ generates the expected compiled program.

### 4. Scoped Arena Allocator & Memory Safety (Phase 44)
- [ ] **MEM-01**: Implement a fast scoped bump-arena runtime in `src/runtime/` with native Cranelift/LLVM lowerings (`__nl_arena_create`, `__nl_arena_alloc`, `__nl_arena_reset`, `__nl_arena_destroy`).
- [ ] **MEM-02**: Lower `box(expr)`, vector allocations, and closure environment allocations to the active scoped arena by default.
- [ ] **MEM-03**: Introduce scoped arena resets at loop headers and function boundaries for temporary allocations, eliminating monotonic memory growth.
- [ ] **MEM-04**: Add memory profiling tests verifying that looping allocation benchmarks (`nrev`, `append3`, closures) run with $O(1)$ peak heap residency rather than unbounded leakage.

### 5. Architecture Decomposition, Codegen Unification & Hardening (Phase 45)
- [ ] **ARCH-01**: Decommission and remove the legacy AST-level supercompiler (`src/opt/supercompiler/`), unifying all supercompilation passes exclusively onto SSA MIR (`src/mir/supercompiler/`).
- [ ] **ARCH-02**: Decompose the 9,306-line `src/codegen/cranelift_backend.rs` into focused submodules: `abi.rs`, `builder.rs`, `intrinsics.rs`, `emit.rs`.
- [ ] **ARCH-03**: Remove duplicate AST-based Cranelift codegen, standardizing the compiler pipeline on `TypedAST -> MIR -> Cranelift/LLVM`.
- [ ] **ARCH-04**: Replace raw `.unwrap()` calls across codegen and typechecker lookups with structured error propagation using `CodegenError` and `TypeError`.
- [ ] **ARCH-05**: Fix unchecked integer arithmetic in `src/mir/supercompiler/generalize.rs` (`d * d`, `c1 + d`, precision loss in `disc as f64`) by using safe integer square roots and checked/saturating arithmetic.

---

## Traceability Matrix

| Requirement | Phase | Status | Target File(s) |
| :--- | :---: | :---: | :--- |
| **PORT-01..04** | Phase 41 | Planned | `src/codegen/cranelift_backend.rs`, `src/codegen/llvm_backend.rs`, `src/codegen/linker.rs`, `src/codegen/entry_bench.c`, `docker/Dockerfile` |
| **LEAN-04..07** | Phase 42 | Planned | `lean/Supercompiler/Semantics.lean`, `lean/Supercompiler/Compaction.lean`, `lean/Supercompiler/Refinement.lean`, `lean/Supercompiler/Main.lean` |
| **FUTA-05..08** | Phase 43 | Planned | `src/stdlib/minspec.nl`, `tests/true_futamura_projections_tests.rs`, `tests/third_futamura_tests.rs` |
| **MEM-01..04** | Phase 44 | Planned | `src/runtime/`, `src/codegen/cranelift_backend.rs`, `src/codegen/llvm_backend.rs`, `tests/heap_supercompile_tests.rs` |
| **ARCH-01..05** | Phase 45 | Planned | `src/opt/supercompiler/` (retire), `src/codegen/cranelift_backend.rs` (decompose), `src/mir/supercompiler/generalize.rs` |
