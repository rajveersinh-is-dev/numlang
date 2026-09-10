# Roadmap: numlang

## Overview

Build numlang from scratch in Rust: starting with lexing, AST construction, and Pratt parsing, advancing through static type verification and semantic analysis, lowering to native machine code via an optimizing backend, providing a developer CLI with mathematical primitives, and establishing a rigorous benchmarking suite against C and Rust.

## Phases

- [x] **Phase 1: Lexer, Parser & AST Diagnostics** - Tokenizer, Pratt expression parser, and AST definition with diagnostic inspection. (completed 2026-09-10)
- [x] **Phase 2: Semantic Analysis & Static Type Checker** - Symbol tables, scope resolution, strict numeric type validation, and error reporting. (completed 2026-09-10)
- [x] **Phase 3: Code Generation & Native Compilation Pipeline** - IR lowering and native Windows x86_64 machine code generation. (completed 2026-09-10)
- [x] **Phase 4: CLI Driver & Numerical Primitives** - User-facing `run` and `build` commands with contiguous array and vector math operations. (completed 2026-09-10)
- [ ] **Phase 5: Benchmark Suite & Optimization Hardening** - Automated comparative performance benchmarks against C and Rust baselines.

## Phase Details

### Phase 1: Lexer, Parser & AST Diagnostics

**Goal**: Build the core syntax parsing engine capable of transforming numlang source code into a structured Abstract Syntax Tree.
**Depends on**: Nothing (first phase)
**Requirements**: LEX-01, LEX-02, LEX-03, LEX-04, CLI-03
**Success Criteria**:

  1. Lexer accurately emits tokens for arithmetic, types, variables, and keywords with line/column spans.
  2. Pratt parser correctly resolves mathematical operator precedence and associativity without ambiguity.
  3. CLI flags `--emit-tokens` and `--emit-ast` print clean, inspectable representation of source code.

**Plans**: 3 plans

Plans:

- [x] 01-01: Cargo project setup, AST definitions, and token/lexer implementation
- [x] 01-02: Pratt expression parser and grammar construction
- [x] 01-03: Diagnostic reporting and AST inspection CLI harness

### Phase 2: Semantic Analysis & Static Type Checker

**Goal**: Implement symbol resolution, variable scope management, and strict static type checking.
**Depends on**: Phase 1
**Requirements**: TYPE-01, TYPE-02, TYPE-03
**Success Criteria**:

  1. Static type mismatches are caught and rejected at compile time.
  2. Immutability violations, undeclared identifiers, and return type mismatches produce clear diagnostic messages.
  3. Valid programs produce a typed AST ready for IR lowering.

**Plans**: TBD

Plans:

- [x] 02-01: Scope environment and symbol table implementation
- [x] 02-02: Static type checker and semantic validation passes

### Phase 3: Code Generation & Native Compilation Pipeline

**Goal**: Lower typed AST into machine intermediate representation (IR) and emit native Windows x86_64 executables.
**Depends on**: Phase 2
**Requirements**: CODEGEN-01, CODEGEN-02, CODEGEN-03
**Success Criteria**:

  1. Arithmetic expressions and variable assignments lower into valid backend IR.
  2. Control flow (`if`/`else`, `while`) and function calls execute correctly.
  3. Standalone executable binaries are generated, linked, and run on Windows x86_64.

**Plans**: TBD

Plans:

- [x] 03-01: Backend IR lowering for arithmetic and functions
- [x] 03-02: Control flow lowering and native object emission/linking

### Phase 4: CLI Driver & Numerical Primitives

**Goal**: Build user-facing compiler CLI commands and implement core numerical primitives (arrays, vectors, intrinsics).
**Depends on**: Phase 3
**Requirements**: CLI-01, CLI-02, MATH-01, MATH-02
**Success Criteria**:

  1. `numlang run <file.nl>` compiles and executes code directly in one command.
  2. `numlang build <file.nl> -o <binary>` outputs optimized native executable.
  3. Contiguous array operations and vector kernels (dot product) compute correct results.

**Plans**: TBD

Plans:

- [x] 04-01: CLI command interface (`build`, `run`, flags)
- [x] 04-02: Contiguous array primitives and vector math intrinsics

### Phase 5: Benchmark Suite & Optimization Hardening

**Goal**: Implement automated performance benchmarking against C and Rust reference implementations.
**Depends on**: Phase 4
**Requirements**: BENCH-01
**Success Criteria**:

  1. Automated test and benchmark runner executes numerical microbenchmarks.
  2. Accurate runtime and memory comparisons are reported against `clang -O3` and `cargo --release`.

**Plans**: TBD

Plans:

- [ ] 05-01: Comparative benchmark suite (dot product, matrix multiply, fibonacci/recursion)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Lexer, Parser & AST Diagnostics | 3/3 | Complete    | 2026-09-10 |
| 2. Semantic Analysis & Static Type Checker | 2/2 | Complete    | 2026-09-10 |
| 3. Code Generation & Native Compilation Pipeline | 2/2 | Complete    | 2026-09-10 |
| 4. CLI Driver & Numerical Primitives | 2/2 | Complete    | 2026-09-10 |
| 5. Benchmark Suite & Optimization Hardening | 0/1 | Not started | - |
