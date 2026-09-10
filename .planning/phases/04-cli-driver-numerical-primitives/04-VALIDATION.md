# Phase 4: CLI Driver & Numerical Primitives - Validation Plan

**Domain:** CLI Subcommands, 1D Contiguous Arrays, Math Intrinsics, Vector Operations
**Confidence:** HIGH

## Verification Commands

```bash
cargo test --test cli_driver_tests
cargo test --test array_math_tests
cargo test
```

## Validation Scenarios

1. **CLI Subcommand `build`:**
   - `numlang build input.nl -o calc.exe` produces executable `calc.exe`.
   - `numlang build input.nl` (without `-o`) defaults to `input.exe`.
2. **CLI Subcommand `run`:**
   - `numlang run input.nl` compiles and executes in one command, forwarding the process exit code.
3. **1D Contiguous Arrays:**
   - Array literal initialization `let a: [i64; 3] = [10, 20, 30];`
   - Array indexing `let x = a[1];` correctly retrieves `20`.
   - Array element assignment `a[0] = 99;` correctly mutates array in-place.
   - Array bounds checking: Accessing `a[5]` on a 3-element array halts with exit code 101.
4. **Vector Math Intrinsics:**
   - `dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0])` produces `32.0` ($1\times4 + 2\times5 + 3\times6 = 32$).
   - `abs(-42)` and `abs(-3.14)` produce positive values.
   - `sqrt(16.0)` produces `4.0`.
