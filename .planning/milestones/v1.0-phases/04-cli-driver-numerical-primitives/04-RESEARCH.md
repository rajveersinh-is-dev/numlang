# Phase 4 Research: CLI Driver & Numerical Primitives

## Context & Objectives
Phase 4 delivers:
1. **CLI Driver**:
   - `numlang run <file.nl>`: Direct execution workflow (compile -> link -> run child process -> propagate exit status).
   - `numlang build <file.nl> [-o <output.exe>]`: AOT compiler producing native executables (default output: `<stem>.exe`).
   - Preserve existing diagnostic flags (`--emit-tokens`, `--emit-ast`, `--emit-ir`, `--emit-obj`, `--check`).
2. **1D Contiguous Array Primitives (`MATH-01`)**:
   - Fixed-size 1D array type: `[T; N]` (e.g., `[f64; 4]`, `[i64; 3]`).
   - Array literals: `[1.0, 2.0, 3.0, 4.0]`.
   - Array indexing (read & write): `let x: f64 = arr[i];` and `arr[i] = 10.0;`.
   - Index bounds checking: At runtime, if index `< 0` or index `>= N`, terminate process safely (e.g. exit code 101 or call abort/ExitProcess) preventing out-of-bounds memory corruption.
3. **Core Math Intrinsics & Vector Operations (`MATH-02`)**:
   - Element-wise operations and math functions: `abs(x)`, `sqrt(x)`.
   - Vector operations:
     - `dot(a, b)`: Computes vector dot product $\sum a_i \cdot b_i$.
     - `vec_add(a, b)`: Element-wise addition of two vectors of matching dimension.
     - `sum(a)`: Computes $\sum a_i$.

## Architecture & Implementation Strategy

### 1. CLI Subcommands with Clap
Use Clap's derive or builder API to support:
```
numlang run <file.nl>
numlang build <file.nl> [-o <output>]
numlang check <file.nl>
```
Or allow both direct file argument and subcommands for seamless backwards compatibility.

### 2. Array Memory Model & Cranelift Codegen
- In Cranelift, a fixed-size array `[T; N]` can be allocated as a `StackSlot` of size $N \times \text{sizeof}(T)$ with alignment matching $T$.
- Indexing `arr[i]`:
  1. Load index value `i`.
  2. Emit bounds check:
     - Is `i < 0` (or `i >= N` as unsigned comparison `icmp(IntCC::UnsignedGreaterThanOrEqual, i, N)`)?
     - If true, branch to panic block (calling `ExitProcess(101)`).
  3. Compute byte offset: `offset = i * sizeof(T)`.
  4. Compute element address: `addr = stack_addr(slot) + offset`.
  5. Load or store at `addr`.

### 3. Built-in Math & Vector Functions
- Can be recognized as built-in functions in `SymbolTable` / `TypeChecker`:
  - `sqrt(f64) -> f64` (lowered to Cranelift `f64.sqrt` / `fsqrt`).
  - `abs(f64) -> f64` (lowered to Cranelift `fabs`) / `abs(i64) -> i64`.
  - `dot(a: [T; N], b: [T; N]) -> T`: Inlined into loop or unrolled product accumulation.
  - `vec_add(a: [T; N], b: [T; N]) -> [T; N]`: Inlined into elementwise sum.
  - `sum(a: [T; N]) -> T`: Inlined into accumulation.

## Validation Strategy
- Integration tests executing `numlang run` via `assert_cmd` / `std::process::Command`.
- Unit and integration tests for array declaration, indexing, mutation, and bounds violation abort.
- Integration test for `dot` product computing correct floating point results and verifying exit codes.
