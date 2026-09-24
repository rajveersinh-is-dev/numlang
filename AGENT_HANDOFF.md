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
| `tests/` | 41 integration test suites. |

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

## 4. Next Milestone: Phase 4 (Coupled Recurrences & Interprocedural Fusion)

- **Goal**: Expand supercompiler to handle multi-variable coupled linear recurrences and interprocedural process-tree exploration.
- **Key Objectives**:
  1. **Coupled Multi-Variable Linear Recurrences**:
     - Handle systems of recurrence equations such as Fibonacci:
       $a_{k+1} = a_k + b_k$, $b_{k+1} = a_k$.
     - Transition matrix diagonalization and $O(\log n)$ matrix exponentiation or Binet closed form.
  2. **Interprocedural Process-Tree Inlining & Specialization**:
     - Drive through function calls when arguments have constant or partially-known symbolic shapes.
     - Cross-function deforestation (eliminating intermediate allocated structures passed across function boundaries).
  3. **Verification**:
     - Tests asserting Fibonacci and mutual recursion collapse to logarithmic or constant time execution.

