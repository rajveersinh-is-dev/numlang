# Requirements: numlang

**Defined:** 2026-09-10
**Core Value:** Delivering high computational throughput and deterministic memory performance for mathematical algorithms with clean, modern syntax.

## v1 Requirements

Requirements for the initial functional compiler release.

### Lexing & Parsing

- [x] **LEX-01**: Lexer tokenizes numeric literals (`i32`, `i64`, `f32`, `f64`), identifiers, mathematical operators (`+`, `-`, `*`, `/`, `%`, `^`), and punctuation.
- [x] **LEX-02**: Lexer tokenizes language keywords (`fn`, `let`, `return`, `if`, `else`, `while`, `for`).
- [x] **LEX-03**: Pratt parser parses mathematical expressions with operator precedence and grouping.
- [x] **LEX-04**: Parser constructs Abstract Syntax Tree (AST) representing function declarations, variable bindings, and control flow blocks.

### Semantic Analysis & Types

- [ ] **TYPE-01**: Type checker verifies static primitive numeric types (`i32`, `i64`, `f32`, `f64`) and prevents implicit lossy conversions.
- [ ] **TYPE-02**: Symbol table enforces variable scoping, immutability defaults, and function signature verification.
- [ ] **TYPE-03**: Diagnostic engine emits human-readable compiler errors with source line and column coordinates.

### Code Generation & Backend

- [ ] **CODEGEN-01**: AST is lowered to Intermediate Representation (LLVM IR / Cranelift) for arithmetic and variable assignments.
- [ ] **CODEGEN-02**: Control flow constructs (`if`/`else`, `while`, loops) and function calls are lowered to machine IR.
- [ ] **CODEGEN-03**: Compiler links and outputs native Windows x86_64 machine executables.

### CLI & Standard Library

- [ ] **CLI-01**: CLI supports `numlang run <file.nl>` for direct compile-and-run execution.
- [ ] **CLI-02**: CLI supports `numlang build <file.nl> -o <binary>` for AOT standalone binary output.
- [x] **CLI-03**: CLI supports diagnostic flags `--emit-tokens`, `--emit-ast`, and `--emit-ir`.
- [ ] **MATH-01**: Built-in 1D contiguous numeric array primitive with index boundary checking.
- [ ] **MATH-02**: Core math intrinsics and SIMD-friendly vector operations (element-wise add, dot product).

### Benchmarks & Validation

- [ ] **BENCH-01**: Automated benchmark harness comparing mathematical kernel execution time against C (`clang -O3`) and Rust (`--release`).

## v2 Requirements

- **OPT-01**: Custom loop auto-vectorization pass for multi-dimensional matrix operations.
- **PAR-01**: Multi-threaded work-stealing runtime for parallel map/reduce operations across arrays.
- **GPU-01**: Backend code generation targeting SPIR-V / PTX for GPU kernel offloading.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Exponential speedups across all benchmarks | Bounded by CPU clock, instruction pipeline, and memory bandwidth physics. |
| Garbage collection runtime | Excluded to preserve deterministic latency and zero runtime overhead. |
| Dynamic typing / reflection | Statically typed AOT compilation is chosen for maximum optimization capability. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| LEX-01 | Phase 1 | Complete |
| LEX-02 | Phase 1 | Complete |
| LEX-03 | Phase 1 | Complete |
| LEX-04 | Phase 1 | Complete |
| CLI-03 | Phase 1 | Complete |
| TYPE-01 | Phase 2 | Pending |
| TYPE-02 | Phase 2 | Pending |
| TYPE-03 | Phase 2 | Pending |
| CODEGEN-01 | Phase 3 | Pending |
| CODEGEN-02 | Phase 3 | Pending |
| CODEGEN-03 | Phase 3 | Pending |
| CLI-01 | Phase 4 | Pending |
| CLI-02 | Phase 4 | Pending |
| MATH-01 | Phase 4 | Pending |
| MATH-02 | Phase 4 | Pending |
| BENCH-01 | Phase 5 | Pending |

**Coverage:**

- v1 requirements: 16 total
- Mapped to phases: 16
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-10*
*Last updated: 2026-09-10 after initial definition*
