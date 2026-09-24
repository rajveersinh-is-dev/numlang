# NumLang v2.0 Roadmap & Technical Architecture

## 1. High-Level Vision

NumLang is being engineered to be the **world's fastest general supercompiler**.
Unlike classical supercompilers (Turchin's Refal, Bolingbroke/Jones's GHC Supercompiler, HSc) which operate on pure functional expression trees and suffer exponential compilation blowup on imperative memory and state transitions, NumLang operates directly on **SSA Control Flow Graphs** via an industrial Mid-level IR (NumLang MIR).

---

## 2. Milestone & Phase Tracker

| Phase | Description | Status | Commit / Notes |
| :--- | :--- | :--- | :--- |
| **Phase 1** | **Core Language & AOT Compiler**<br>Lexer, Pratt parser, Type checker, Win32 Cranelift backend, vectorization & math intrinsics | **Completed** | Full language v1.0, 35+ test suites, sub-millisecond execution |
| **Phase 2.1** | **MemorySSA Token Graph**<br>Explicit token-versioned memory state (`MemoryVersionId`), memory $\phi$-nodes, while-loop memory merges | **Completed** | Commit `1184127` (`src/mir/memory_ssa.rs`) |
| **Phase 2.2** | **Field-Sensitive Alias Analysis**<br>Exact offset tracking for distinct locals and struct fields, constant array indexing disambiguation | **Completed** | Commit `a2d4cb4` (`src/mir/alias.rs`) |
| **Phase 2.3** | **Mem2Reg SSA Promotion, RLE & DSE**<br>Iterated Dominance Frontier (IDF) promotion of memory variables to pure SSA block parameters, redundant load elimination, dead store elimination | **Completed** | Commit `825fd2d` (`src/mir/mem2reg.rs`) |
| **Phase 2.4** | **Verification Gate & CLI**<br>`--emit-memory-ssa`, integration test suite across complex CFGs | **Completed** | Commit `96a48e3` (`tests/memory_ssa_tests.rs`) |
| **Phase 2.5** | **General SSA Process-Tree Supercompiler**<br>Hash-consed terms (`term.rs`), path constraint store (`state.rs`), topological whistle & homeomorphic embedding (`whistle.rs`), closed-form recurrence solver (`generalize.rs`), residualization (`residualize.rs`), deforestation | **Completed** | Commits `8cac976`, `63dd476` (`tests/supercompiler_symbolic_tests.rs`, `tests/deforestation_tests.rs`) |
| **Phase 3** | **MIR to Native Cranelift Codegen Hookup**<br>Directly compiling residualized `MirProgram` to native machine code via Cranelift, enabling supercompiled binaries in `numlang build` and `numlang run` | **Completed** | Full Cranelift MIR lowering, `--supercompile` & `--use-mir` CLI flags, `compile_mir_to_obj`, `compile_supercompiled_to_obj`, `tests/mir_codegen_tests.rs` |
| **Phase 4** | **Coupled Recurrences & Interprocedural Fusion**<br>Multi-variable linear recurrences (Fibonacci/Lucas, Tribonacci, coupled state matrices via $O(\log n)$ matrix exponentiation and roots), caller-callee process-tree inlining & cross-procedural loop fusion | **Completed** | Full order-2 & order-3 recurrence solvers, interprocedural driving, Cranelift `__numlang_fib` intrinsic codegen, `tests/coupled_recurrence_and_fusion_tests.rs` |
| **Phase 5** | **Empirical Benchmark Suite vs State-of-the-Art**<br>Reproducible automated harness comparing NumLang against HSc, Refal-5, Supero, `clang -O3`, and `rustc -O` | **Planned** | Benchmark proof |

---

## 3. Architecture Deep-Dive: The SSA Process Tree

```
                     Source (.nl)
                          │
                          ▼
                     Type Checker
                          │
                          ▼
                     MIR Lowering (`src/mir/lower.rs`)
                          │
                          ▼
            Mem2Reg & MemorySSA (`src/mir/mem2reg.rs`)
                          │
                          ▼
           SSA Supercompiler Driver (`src/mir/supercompiler/drive.rs`)
             ├── Positive Constraint Propagation (`state.rs`)
             ├── Topological Whistle & Embedding (`whistle.rs`)
             ├── Closed-Form Recurrence Solver (`generalize.rs`)
             │    ├── Degree 1: Linear ($O(1)$)
             │    ├── Degree 2: Triangular / Quadratic ($O(1)$)
             │    ├── Degree 3: Cubic ($O(1)$)
             │    └── Geometric ($O(1)$)
             └── Knot-Tying & Folding (`drive.rs`)
                          │
                          ▼
            Residual SSA CFG Generation (`residualize.rs`)
                          │
                          ▼
            Cranelift Native Codegen (`src/codegen/cranelift_backend.rs`)
                          │
                          ▼
                Native Standalone Binary (.exe)
```

---

## 4. Key CLI Flags

- `numlang run <file.nl>`: Compiles and executes native binary in-memory.
- `numlang build <file.nl> -o <out.exe>`: Links standalone Windows executable.
- `numlang --emit-mir <file.nl>`: Displays initial MIR SSA CFG.
- `numlang --emit-memory-ssa <file.nl>`: Displays MemorySSA token version graph.
- `numlang --emit-supercompiled-mir <file.nl>`: Displays supercompiled residual MIR CFG.
- `numlang --emit-process-tree <file.nl>`: Displays full process tree graph with states, transitions, and knot edges.
- `numlang --supercompile-stats <file.nl>`: Reports metrics on nodes explored, branches pruned, loops collapsed to $O(1)$, and knots tied.
- `numlang --use-mir-codegen <file.nl>`: Forces execution through the direct MIR Cranelift backend.

---

## 5. Development Invariants

1. **Pure Rust**: Zero external C/C++ dependencies; builds portably via standard `cargo build`.
2. **Clippy Clean**: Must always pass `cargo clippy --all-targets -- -D warnings` with zero warnings.
3. **100% Green Tests**: All existing test suites (currently 40+ suites) must pass before committing changes.
