# NumLang v1.0 -> v2.0 Engineering Handoff Document

## 1. Executive Summary & Current State

- **Repository Root**: `C:\Users\davea\.gemini\antigravity\scratch\numlang`
- **Git Branch**: `master` (All 13 phases of v1.0 committed, clean working tree)
- **Latest Release Binary**: `target\release\numlang.exe`
- **Test Status**: **100% GREEN** across all 36 test files (130+ unit and integration tests, passes in ~6.5 seconds via `cargo test --tests`)
- **Lint Status**: **0 WARNINGS / 0 ERRORS** across all targets (`cargo clippy --all-targets -- -D warnings`)
- **Benchmark Parity**: **100% Exit-Code Parity** against Rust (`rustc -O3 -C target-cpu=native`) and C (`cl.exe /O2`) on:
  - 5 Novel Supercompiler Benchmarks (`scratch/run_novel_benchmarks.py`)
  - 5 Difficult Benchmarks (`scratch/test_difficult.py`)
  - 10 Canonical Supercompiler Workloads (`scratch/run_supercompiler_comparison.py`)

---

## 2. Environment Rules & Windows Constraints

When executing commands or modifying code in this repository:
1. **POWERSHELL SYNTAX**: The default shell is PowerShell on Windows. **NEVER use `&&` to chain commands** (PowerShell syntax error). Use `;` or run commands sequentially.
2. **NO UNNECESSARY SUBAGENTS**: Run compiler commands, test runs, and code edits directly in the main turn to prevent context drift and subagent stalls.
3. **MSVC BUILD TOOLS**: If compiling C benchmarks or invoking the MSVC linker, `vcvars64.bat` is located at:
   `C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat`
4. **STRICT DISCIPLINE**: Never leave the repository in a non-compiling or failing test state. Always run:
   - `cargo test --tests`
   - `cargo clippy --all-targets -- -D warnings`
   before committing each phase.
5. **COMMIT DISCIPLINE**: Exactly one git commit per completed phase with a clear descriptive message.

---

## 3. Architecture Overview of the Codebase

- `src/token.rs`: Token definitions (Logos lexer tokens, keywords, operators, doc-comments).
- `src/ast.rs`: Untyped Abstract Syntax Tree.
- `src/lexer/`: Lexical analysis using `logos`. Handles doc comments `///` and skips regular comments `//`.
- `src/parser/`: Pratt-precedence parser for expressions, statements, structs, functions, loops, and match patterns.
- `src/typecheck/`:
  - `checker.rs`: Bidirectional type checker with symbol table scoping.
  - `symtab.rs`: Symbol environment (`Symbol`, `FunctionSig`).
  - `types.rs`: Complete type system (`i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `usize`, `f32`, `f64`, `bool`, `[T; N]`, structs).
  - `typed_ast.rs`: Type-annotated AST (`TypedProgram`, `TypedStmt`, `TypedExpr`).
- `src/opt/`:
  - `mod.rs`: Fixpoint optimizer pipeline (up to 4 passes of inlining, constant folding, BCE, SROA, loop optimization, and supercompilation).
  - `supercompiler/`:
    - `driver.rs`: Partial evaluator and symbolic driver with bounded execution (`MAX_CONCRETE_STEPS = 100_000`, `SNAPSHOT_LIMIT = 256`).
    - `generalization.rs`: Derives degree 0-4 polynomial closed forms via forward difference tables ($\binom{n}{k} \Delta^k y_0$) and $O(k^3 \log N)$ coupled linear recurrences via Gauss-Jordan matrix exponentiation.
    - `termination.rs`: Homeomorphic embedding ($\trianglelefteq$) whistle to guarantee termination.
    - `value.rs`: Symbolic and concrete value algebra.
    - `residualizer.rs`: Emits closed-form return values or residual AST.
  - `bce.rs`: Bounds-check elimination.
  - `sroa.rs`: Scalar Replacement of Aggregates.
  - `inlining.rs`: Function inlining.
  - `while_unroll.rs`: Small loop unrolling and dead-branch pruning.
- `src/codegen/`:
  - `cranelift_backend.rs`: Cranelift AOT/JIT backend lowering typed AST to native x86-64 machine code.
  - `linker.rs`: Robust cross-platform linker (scans Windows SDK, MSVC `link.exe`, `rust-lld`, or Unix `cc`/`clang`).
- `src/fmt.rs`: Official NumLang AST pretty-printer (`numlang fmt`).
- `src/doc.rs`: Markdown documentation generator (`numlang doc`).
- `src/diagnostic.rs`: Error codes `E001`..`E020` and `numlang explain <CODE>`.

---

## 4. Benchmark & Verification Scripts

The `scratch/` directory contains automated verification suites:
1. `python scratch/run_novel_benchmarks.py`: 5 novel supercompiler benchmarks (Polynomial progression, 5-state automaton, geometric power of three, leapfrogging, Collatz peak).
2. `python scratch/test_difficult.py`: 5 difficult benchmarks (Coupled Tribonacci, dynamic prime sieve, Rule 110, Takeuchi deep recursion, cubic modular series).
3. `python scratch/run_supercompiler_comparison.py`: Comprehensive 10-workload benchmark comparing NumLang against Rust/LLVM (`-O3 -C target-cpu=native`) and MSVC C (`/O2`).

To test all 25 multi-language workloads:
```powershell
cargo test --test multi_language_benchmarks -- --ignored
```

---

## 5. Next Mission: NumLang v2.0 Architecture

The user has requested to elevate NumLang into **the best general-purpose compiler in the world**.
See **`PROMPT_V2.md`** for the full 10-phase specification.

### Immediate Next Step: Phase 1 (High-Level Mid-Level Intermediate Representation / MIR)
The next agent should begin with **Phase 1 of `PROMPT_V2.md`**:
- Create `src/mir/mod.rs`, `src/mir/dominance.rs`, `src/mir/lower.rs`.
- Design the basic-block Control Flow Graph (CFG) in SSA form.
- Implement AST-to-MIR lowering and add the CLI flag `--emit-mir` to `src/main.rs`.
- Ensure all 36 test files remain 100% green and Clippy remains 0 warnings.
