# NumLang Agent Handoff Document

This document enables any AI agent or human contributor to instantly take over development on NumLang without losing context.

---

## 1. Project Location & Build Commands

- **Root Directory**: `C:\Users\davea\.gemini\antigravity\scratch\numlang`
- **Build**: `cargo build`
- **Test All**: `cargo test --tests`
- **Lint Check**: `cargo clippy --all-targets -- -D warnings` (Strict requirement: 0 warnings allowed)

---

## 2. Codebase Organization

| Directory / File | Description |
| :--- | :--- |
| `src/ast.rs` | AST definitions (`Expr`, `Stmt`, `Program`, `BinaryOp`, `UnaryOp`). |
| `src/token.rs` | Logos-based lexer definitions and tokens. |
| `src/parser/` | Pratt parser (`expr.rs`, `stmt.rs`, `mod.rs`). |
| `src/typecheck/` | Type checker (`types.rs`, `typed_ast.rs`, `checker.rs`). |
| `src/mir/` | Mid-level IR representations and SSA optimizations: |
| `src/mir/lower.rs` | Lowers `TypedProgram` to `MirProgram` (`MirFunction`, `MirBasicBlock`, `Statement`, `Rvalue`, `Terminator`). |
| `src/mir/dominance.rs` | Dominator tree, immediate dominators, dominance frontiers, and loop header detection. |
| `src/mir/memory_ssa.rs` | Token-versioned `MemorySSA` graph for alias tracking and memory $\phi$-nodes. |
| `src/mir/alias.rs` | Field-sensitive alias analysis (`MustAlias`, `MayAlias`, `NoAlias`). |
| `src/mir/mem2reg.rs` | Iterated Dominance Frontier (IDF) scalar promotion, RLE, and DSE. |
| `src/mir/supercompiler/` | **General SSA Process-Tree Supercompiler**: |
| `├── term.rs` | Hash-consed symbolic term graph (`SymTermId`, `SymTerm`, `TermInterner`) with algebraic simplification. |
| `├── state.rs` | `SymbolicState` (environment mapping places to terms) and `PathConstraintStore`. |
| `├── whistle.rs` | Fast homeomorphic embedding whistle ($\trianglelefteq$) with $O(1)$ size filters. |
| `├── generalize.rs` | Closed-form recurrence solver (linear, quadratic/triangular, cubic, geometric) and MSG. |
| `├── drive.rs` | `SupercompilerDriver`, process tree unfolding, branch splitting, knot-tying, and stats tracking. |
| `├── residualize.rs` | Transforms `ProcessTree` back into optimized SSA `MirFunction`. |
| `└── mod.rs` | Public supercompiler entry points (`supercompile_mir_program`, `supercompile_mir_function`). |
| `src/codegen/` | Cranelift JIT/AOT native code generation and Windows PE linker (`cranelift_backend.rs`, `linker.rs`). |
| `src/main.rs` | Compiler CLI entry point and pipeline driver. |
| `tests/` | 42 integration test suites. |

---

## 3. Completed: Phase 3 (MIR Codegen Hookup & Supercompiler Pipeline)

- **Status**: Completed (100% tests passing, 0 Clippy warnings).
- **Deliverables**:
  - `compile_mir_to_obj(mir: &MirProgram) -> Result<Vec<u8>, CodegenError>` in `src/codegen/cranelift_backend.rs`.
  - `compile_supercompiled_to_obj(program: &TypedProgram) -> Result<Vec<u8>, CodegenError>` in `src/codegen/cranelift_backend.rs`.
  - Full Cranelift SSA lowering: blocks, branches (`jump`, `brif`, `switch`), binary ops with signed/unsigned/float coercion, dummy return safety.
  - Added `SymTerm::Call` across supercompiler pipeline (`term.rs`, `whistle.rs`, `generalize.rs`, `drive.rs`, `residualize.rs`).
  - Added CLI flags: `--supercompile` and `--use-mir` on `numlang build` and `numlang run`.
  - Integration tests in `tests/mir_codegen_tests.rs` (6/6 tests passing).

---

## 4. Completed: Phase 4 (Coupled Recurrences & Interprocedural Fusion)

- **Status**: Completed (100% tests passing, 0 Clippy warnings).
- **Deliverables**:
  - **Coupled Multi-Variable Linear Recurrence Solvers** (`src/mir/supercompiler/generalize.rs`):
    - `solve_order2_recurrence`: Solves $s_k = c_1 s_{k-1} + c_2 s_{k-2}$ via $2 \times 2$ matrix system and Cramer's rule.
      - Constant iterations ($N$ known): computes exact value in $O(\log N)$ via `mat_pow_2x2`.
      - Integer characteristic roots: derives closed form $A \cdot r_1^n + B \cdot r_2^n$.
      - Irrational characteristic roots (Fibonacci, Lucas): detects $c_1=1, c_2=1$ and emits `__numlang_fib` intrinsic term.
    - `solve_order3_recurrence`: Solves $s_k = c_1 s_{k-1} + c_2 s_{k-2} + c_3 s_{k-3}$ (e.g. Tribonacci) via $3 \times 3$ Cramer's rule and $O(\log N)$ `mat_pow_3x3`.
  - **Interprocedural Process-Tree Driving & Inlining** (`src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/mod.rs`):
    - `try_drive_interprocedural_call`: specializes and executes callee functions with argument terms, extracting unified return terms and importing them back into caller state.
    - `TermInterner::import_from`: imports folded/algebraic terms across interprocedural driver instances.
    - Cross-procedural loop fusion: interprocedurally driven calls allow caller and callee loop nests to fuse and collapse to $O(1)$ closed forms.
  - **Reflexive Comparison Simplifications** (`src/mir/supercompiler/term.rs`, `src/mir/supercompiler/state.rs`):
    - Handled $x < x \to \text{false}$, $x > x \to \text{false}$, $x \le x \to \text{true}$, $x \ge x \to \text{true}$ to resolve loop boundary conditions when induction variables hit upper bounds.
  - **Cranelift Native MIR Codegen for Recurrences** (`src/codegen/cranelift_backend.rs`):
    - Native SSA loop generation for `__numlang_fib` intrinsic calls in `compile_mir_function`.
  - **Integration Test Suite** (`tests/coupled_recurrence_and_fusion_tests.rs`):
    - 7/7 tests passing covering direct order-2 Fibonacci, integer roots, Tribonacci, interprocedural inlining, triangular loop fusion, and native execution of both concrete and symbolic coupled recurrences.

---

## 5. Next Milestone: Phase 5 (Empirical Benchmark Suite vs State-of-the-Art)

- **Goal**: Empirically prove NumLang's supercompiler performance and asymptotic advantages over classical supercompilers (HSc, Refal-5) and optimizing production compilers (`clang -O3`, `rustc -O`).
- **Key Objectives**:
  1. Build automated benchmark harness with cycle/nanosecond timing and memory allocation profiling.
  2. Implement canonical benchmark programs:
     - Ackermann function with constant/small arguments (deforestation & constant folding).
     - Fibonacci / Tribonacci loops ($O(N)$ transformed to $O(1)$ / $O(\log N)$).
     - Triangular & polynomial summations ($\sum i$, $\sum i^2$, $\sum i^3$).
     - Multi-pass array transformations (map-filter-reduce fusion).
     - String / sequence pattern matching with KMP-like DFA synthesis.
  3. Generate comparative markdown reports and speedup charts showing speedup ratios against Clang, GCC, Rust, and HSc.

---

## 6. Completed: Phase 10 (Close All Turchin Supercompiler Gaps)

- **Status**: Completed (100% tests passing, 0 Clippy warnings under `-D warnings`).
- **Tracks & Deliverables**:
  1. **Heap Pointer Type Plumbing** (Track 5):
     - Added `Type::Ptr(Box<Type>)` to `src/typecheck/types.rs`.
     - Plumbed layout, size, alignment, and native pointer lowering across `src/codegen/cranelift_backend.rs` and `src/codegen/llvm_backend.rs`.
     - Added `SymTerm::Ref(SymTermId, Type)` and `SymTerm::Deref(SymTermId, Type)` to `src/mir/supercompiler/term.rs`, `whistle.rs`, `generalize.rs`, and `residualize.rs`.
  2. **Negative Discriminant Propagation in MIR Switch** (Track 3):
     - Added `excluded_values: HashMap<SymTermId, Vec<i64>>`, `add_not_equal_int`, `deduce_must_equal_int`, and negative constraint propagation to `src/mir/supercompiler/state.rs`.
     - In `src/mir/supercompiler/drive.rs`, propagated negative discriminant constraints across all arms and default arms of `Terminator::Switch`.
     - Added `test_negative_discriminant_propagation` in `tests/futamura_projection_tests.rs`.
  3. **Partial Specialization in AST Driver** (Track 1):
     - Extended `src/opt/supercompiler/value.rs` with `sym_to_expr`, `value_to_typed_expr`, `contains_opaque()`, and `is_symbolic_opaque()`.
     - Handled `__lit_` unstripping, desugared `SymExpr::If` to `TypedExpr::Match`, and preserved symbolic returns in `src/opt/supercompiler/driver.rs` and `mod.rs`.
     - Added `test_partial_specialization_symbolic_arg` in `tests/futamura_projection_tests.rs`.
  4. **Coupled 2-Variable Linear Recurrence Solver via Matrix Exponentiation** (Track 2):
     - Implemented `solve_coupled_2var_recurrence` in `src/mir/supercompiler/generalize.rs` supporting homogeneous and affine coupled systems via Cramer's rule and `mat_pow_3x3`.
     - Registered `__coupled_a` and `__coupled_b` builtins in `src/typecheck/checker.rs` and added native Cranelift loop lowerings in `src/codegen/cranelift_backend.rs` (both AST and MIR).
     - Hooked multi-variable candidate matching into `try_solve_loop_recurrence` in `src/mir/supercompiler/drive.rs`.
     - Added `test_coupled_2var_recurrence_direct` and `test_coupled_2var_linear_recurrence` in `tests/coupled_recurrence_and_fusion_tests.rs`.
  5. **2nd Futamura Projection Verification** (Track 4):
     - Added `test_2nd_futamura_projection` to `tests/futamura_projection_tests.rs` specializing an arithmetic AST interpreter on a static program expression with symbolic runtime arguments.
     - Formally asserted zero residual interpreter dispatch (`TypedExpr::Match`) and zero calls to `eval`, collapsing directly to target arithmetic.
     - Verified runtime execution and correctness across test inputs ($x=5 \to 16$, $x=10 \to 26$, $x=0 \to 6$).



