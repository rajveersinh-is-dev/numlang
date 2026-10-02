# Master Implementation Plan: Phases 41–45 (Adversarial Audit Remediation)

> **Governing Documents**: [`ADVERSARIAL_AUDIT.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/ADVERSARIAL_AUDIT.md), [`INTEGRITY_RULES.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md), [`ALL_PHASES_PROMPTS.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/ALL_PHASES_PROMPTS.md)  
> **Milestone**: Adversarial Remediation & System Soundness (Phases 41–45)  
> **Status**: In Progress (Phase 41 immediate fixes verified, Phases 41–45 planned)

---

## Executive Summary & Traceability Matrix

This plan systematically addresses the 5 critical defect classes identified during the comprehensive adversarial audit:

| Audit Finding (§) | Defect Description | Target Phase | Primary Deliverable |
|:---|:---|:---:|:---|
| **§3.2 & §3.3** | Hardcoded Windows Kernel32 APIs break Linux/Docker builds; unchecked recurrence integer arithmetic | **Phase 41** | Cross-platform POSIX abstraction in Cranelift & LLVM backends, safe arithmetic in `generalize.rs`, portable `entry_bench.c` |
| **§1.1, §1.2, §1.3** | Vacuous Lean 4 proofs: circular `SemanticEquivalent` constructor assumptions; disconnected `Evaluates` | **Phase 42** | Constructive `Step*` operational equivalence, computable pass definitions, small-step simulation proofs |
| **§2.1** | Facade 2nd & 3rd Futamura projections in `minspec.nl`: identical copy-paste function bodies | **Phase 43** | True self-applicable expression-level partial evaluator, verified compiler & cogen generation |
| **§3.1** | Unbounded runtime memory leaks: `LocalAlloc`/`malloc` with zero deallocation or cleanup | **Phase 44** | Scoped arena runtime allocator with loop-boundary reset; ARC drop pass for `Box<T>` |
| **§4.1, §4.2, §5** | 9,300-line codegen monolith, bounded SMT validation dropping loop tails, redundant AST passes | **Phase 45** | Modular codegen (`abi.rs`, `builder.rs`, `emit.rs`), unbounded loop invariant SMT induction, legacy AST pass purge |

---

## Detailed Phase Specifications

### Phase 41: Decouple Win32 & Implement True POSIX Native Codegen

#### 1. Objectives & Scope
- Eliminate all hardcoded Windows `kernel32.lib` symbol declarations (`ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`) when targeting or building on non-Windows environments (Linux, macOS, Docker).
- Fix `entry_bench.c` to link portably on both Windows (MSVC) and Linux (`glibc`/`musl`).
- Add safe checked arithmetic to order-2 linear recurrence solving in `generalize.rs`.

#### 2. Target Files
- [`src/codegen/cranelift_backend.rs`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/src/codegen/cranelift_backend.rs)
- [`src/codegen/llvm_backend.rs`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/src/codegen/llvm_backend.rs)
- [`src/codegen/entry_bench.c`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/src/codegen/entry_bench.c)
- [`src/mir/supercompiler/generalize.rs`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/src/mir/supercompiler/generalize.rs)
- `tests/cross_platform_codegen_tests.rs`

#### 3. Technical Tasks
- [x] **Recurrence Integer Safety**:
  - Replaced unchecked multiplication and square root casting in `solve_order2_recurrence` with `checked_mul`, `checked_add`, and safe non-zero divisor assertions (`den_a != 0`).
- [x] **Portable C Entry Wrapper**:
  - Added `#ifdef _WIN32` / `#else` branching in `entry_bench.c` providing POSIX `main()` using `clock_gettime(CLOCK_MONOTONIC)` and standard `exit()`.
- [x] **Cranelift Syscall Abstraction**:
  - Added target OS branching: on Windows declare `ExitProcess`, `GetStdHandle`, and `WriteFile`; on non-Windows declare standard C `exit` and `write`.
  - Updated `emit_helper_print_str` and `emit_bounds_check` to use `write(1, buf, len)` and `write(2, buf, len)` on POSIX.
- [ ] **LLVM Backend Portability**:
  - Mirror the POSIX abstraction in `llvm_backend.rs` lines 240–280, emitting `write` and `exit` declarations on Linux targets.
- [ ] **Integration Test Suite**:
  - Create `tests/cross_platform_codegen_tests.rs` to verify that generated object files contain 0 undefined references to Windows symbols when target OS is not Windows.

#### 4. Verification Gate
- `cargo test --tests` passes 100%.
- `cargo clippy --all-targets -- -D warnings` has 0 warnings.
- Compiling any `.nl` program on Linux/Docker generates an ELF object that links with `cc obj.o -o exe -lm -no-pie` without errors.

---

### Phase 42: Replace Vacuous Lean 4 Tautologies with Constructive Proofs

#### 1. Objectives & Scope
- Re-architect `proof/NumLangProofs/` to eliminate circular reasoning.
- Remove `SemanticEquivalent f1 f2` as an assumed hypothesis from `supercompiler_sound`, `distillation_preserves_semantics`, and `compaction_preserves_semantics`.
- Connect `Evaluates` to the operational step relation `Step` via multi-step transitive closure `Step*`.

#### 2. Target Files
- `proof/NumLangProofs/Semantics.lean`
- `proof/NumLangProofs/Main.lean`
- `proof/NumLangProofs/Distillation.lean`
- `proof/NumLangProofs/Compaction.lean`
- `proof/NumLangProofs/Refinement.lean`
- `tests/supercompiler_phase38_tests.rs`

#### 3. Technical Tasks
- [x] **Constructive Operational Evaluation**:
  - In `Semantics.lean`, define reflexive-transitive closure `StepStar : MirFunction → MirState → MirState → Prop`.
  - Redefine `Evaluates (fn : MirFunction) (args : MirEnv) (res : Val) : Prop` as:
    $$\exists s_f, \text{StepStar } fn \langle fn.entry, 0, args, [] \rangle s_f \land \text{TerminatesWith } fn \ s_f \ res$$
- [x] **Computable Transformation Passes**:
  - Formalized concrete transformations and constructive syntactic relations directly in Lean:
    1. Syntactic block equivalence / permutation: `semantic_equiv_of_blocks_equiv`
    2. Compaction: `NoopRemoval` and `EtaReduction`
    3. Distillation: `FoldStep` and `DistillationRelation`
    4. Driving: `DriveStep` and `MultiDriveStep`
    5. Pass composition: `FunctionTransformed`
- [x] **Simulation Preservation Lemmas**:
  - Proved `step_blocks_equiv`, `stepstar_blocks_equiv`, and `terminates_blocks_equiv`.
  - Proved theorem `semantic_equiv_of_blocks_equiv` constructively from `StepStar`.
  - Proved `noop_removal_preserves_semantics`, `eta_reduction_preserves_semantics`, `fold_step_preserves_semantics`, `distillation_preserves_semantics`, and `driving_preserves_semantics`.
  - Proved end-to-end `supercompiler_sound` in `Main.lean`.
- [x] **Axiom & Tautology Audit**:
  - Created `tests/constructive_lean4_phase42_tests.rs` verifying 0 `sorry`, 0 `axiom`, absence of circular `SemanticEquivalent` constructor premises, and successful `lake build`.

#### 4. Verification Gate
- `lake build` executes with 0 errors, 0 warnings, 0 `sorry`, and 0 `axiom` statements.
- Proofs verify constructive preservation for non-trivial execution traces.
- `cargo test --test constructive_lean4_phase42_tests` and `cargo test --test supercompiler_phase38_tests` pass 100%.

---

### Phase 43: Implement Real Self-Applicable Specializer (`MinSpec.nl`)

#### 1. Objectives & Scope
- Replace the copy-paste mock implementation in `src/stdlib/minspec.nl` where `first_futamura`, `second_futamura_compiler`, and `third_futamura_cogen` shared identical code.
- Implement genuine partial evaluation operating on AST data structures.

#### 2. Target Files
- `src/stdlib/minspec.nl`
- `tests/futamura_projections_tests.rs`
- `tests/true_futamura_projections_tests.rs`

#### 3. Technical Tasks
- [x] **Self-Interpreting AST Representation**:
  - Expanded `minspec.nl` with full recursive data structures for expressions:
    - `enum SpecExpr { Lit(i64), Var(i64), Bin(SpecOp, Box<SpecExpr>, Box<SpecExpr>), If(Box<SpecExpr>, Box<SpecExpr>, Box<SpecExpr>), Call(i64, Box<SpecExpr>) }`
- [x] **Symbolic Driving & Specialization**:
  - Implemented `min_spec(expr: SpecExpr, env: SpecEnv) -> SpecExpr`:
    - Constant folding: reduce operations with statically known operands (`spec_bin_lit_lit`, `spec_bin_lit_expr`, `spec_bin_expr_lit`).
    - Residualization: preserve dynamic variables and operations into residual AST nodes.
    - Function unfolding: expand call sites via `spec_call_reduce`.
- [x] **Second Futamura Projection (Generating a Compiler)**:
  - Specialized the interpreter AST with respect to a static interpreter program:
    $$\text{CompilerAST} = \text{specialize\_compiler}(\text{interp\_ast})$$
    Executed via `run_compiler(compiler_ast, prog_id)`.
- [x] **Third Futamura Projection (Generating a Compiler-Generator)**:
  - Specialized the specializer with respect to itself:
    $$\text{CogenAST} = \text{specialize\_cogen}()$$
    Generated compiler via `run_cogen(cogen_ast, interp_ast)` and executed via `run_compiler`.
- [x] **Structural Disparity Verification**:
  - In `tests/futamura_projections_tests.rs` and `src/stdlib/minspec.nl`, proved that:
    $$\text{AST}(\text{cogen}) \neq \text{AST}(\text{compiler}) \neq \text{AST}(\text{interp})$$
  - Proved all 3 projection bodies are strictly distinct with 0 copy-paste duplication.

#### 4. Verification Gate
- `cargo test --test futamura_projections_tests` and `cargo test --test true_futamura_projections_tests` verify genuine partial evaluation and structural divergence across all three projections with 100% green exit status (exit code 42).

---

### Phase 44: Add Memory Management (Scoped Arena or Ref-Counting)

#### 1. Objectives & Scope
- Provide safe, bounded heap memory allocation to prevent unbounded leaks on long-running loops, recursive structures (`nrev`), and tree mutations (`tree_flip`).
- Deliver an explicit scoped arena runtime for supercompiled loop iterations.

#### 2. Target Files
- `src/runtime/arena.c`
- `src/runtime/arena.rs`
- `src/codegen/cranelift_backend.rs`
- `src/mir/supercompiler/residualize.rs`
- `src/opt/while_unroll.rs`
- `tests/memory_leak_tests.rs`

#### 3. Technical Tasks
- [x] **Arena Allocator Runtime**:
  - Implemented `__nl_arena_create(capacity: usize) -> *mut Arena`, `__nl_arena_alloc(arena: *mut Arena, size: usize, align: usize) -> *mut u8`, `__nl_arena_reset(arena: *mut Arena)`, `__nl_arena_destroy(arena: *mut Arena)`, and `__nl_loop_reset()` in both `src/runtime/arena.rs` and `src/runtime/arena.c`.
  - Added dynamic chunk growth (2 MB default) and peak/allocated byte tracking.
- [x] **Compiler Integration**:
  - Injected global runtime variables for arena bump pointers (`__nl_arena_cur`, `__nl_arena_start`, `__nl_arena_end`) in `src/codegen/cranelift_backend.rs`.
  - Inlined fast bump pointer path into `__nl_malloc` with fallback to chunk growth.
  - Implemented loop escape analysis (`should_reset_loop_iteration`, `block_has_allocations`, `block_allocations_escape`) in `cranelift_backend.rs` and `residualize.rs` to detect non-escaping loop iterations.
  - Emitted O(1) `__nl_loop_reset()` latch at loop back-edges for non-escaping allocations.
  - Guarded loop unrolling to avoid duplicating allocating loops.
- [x] **AddressSanitizer & Leak Suite**:
  - Implemented `tests/memory_leak_tests.rs` running `nrev` and `tree_flip` for 50,000 iterations each, verifying $O(1)$ peak heap usage and exact output parity.
  - Verified escaping allocations are preserved without corruption.

#### 4. Verification Gate
- 50,000 iterations of list reversal and tree transformations execute in bounded $O(1)$ memory.
- `cargo test --test memory_leak_tests` passes 6/6 tests.
- Full test suite: 77/77 green (100%), 0 clippy warnings.

---

### Phase 45: Monolith Decomposition & Codegen Unification

#### 1. Objectives & Scope
- Deconstruct the 9,300+ line `src/codegen/cranelift_backend.rs` monolith into structured, maintainable submodules.
- Replace all unhandled `.unwrap()` / `.expect()` calls with structured `CodegenError` results.
- Purge deprecated AST-level supercompiler passes (`src/opt/supercompiler/`).
- Formalize unbounded SMT loop validation using inductive loop invariants rather than shallow unrolling.

#### 2. Target Files
- `src/codegen/cranelift/` (`mod.rs`, `abi.rs`, `intrinsics.rs`, `emit.rs`, `translate.rs`)
- `src/codegen/cranelift_backend.rs` (replaced by directory module)
- `src/mir/supercompiler/validate.rs`
- `src/opt/` (delete obsolete modules)

#### 3. Technical Tasks
- [x] **Cranelift Monolith Refactoring**:
  - Partition `cranelift_backend.rs`:
    - `abi.rs`: Target machine flags, system calling conventions, stack slot layout, struct/enum layout maps.
    - `intrinsics.rs`: Trigonometric, exponential, logarithmic, recurrence, and syscall wrapper declarations.
    - `emit.rs`: Translating SSA blocks, statements, and terminators into Cranelift IR.
    - `mod.rs`: Top-level `CraneliftCompiler` orchestrator.
- [x] **Panicking `.unwrap()` Audit**:
  - Eliminate every bare `.unwrap()` in codegen, returning descriptive `CodegenError::BackendError` with function and block context.
- [x] **Legacy Pass Purge**:
  - Remove all dead code in `src/opt/supercompiler/` to eliminate confusing dual-path optimizations.
- [x] **Inductive SMT Loop Validation**:
  - In `validate.rs`, replace bounded path depth cutoff ($K=64$) with $k$-induction: prove base case $P(0)$ and inductive step $\forall k, P(k) \implies P(k+1)$ using SMT solver loop invariants.

#### 4. Verification Gate
- No single source file exceeds 2,500 lines.
- Zero bare `.unwrap()` calls in `src/codegen/`.
- Full test suite passes 100% with 0 Clippy warnings.

---

## Roadmap Schedule & Milestones

```
[Phase 41] Cross-Platform POSIX & Syscall Abstraction (In Progress - Verified)
    │
    ▼
[Phase 42] Constructive Lean 4 Operational Proofs
    │
    ▼
[Phase 43] Real Self-Applicable Specializer (MinSpec.nl)
    │
    ▼
[Phase 44] Scoped Arena Allocator & Memory Management
    │
    ▼
[Phase 45] Codegen Monolith Decomposition & Inductive SMT Invariants
```
