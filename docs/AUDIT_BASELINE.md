# Audit Baseline

## 1. Toolchain Results

### `cargo build --release`
- **Result:** [PENDING]
- **Wall time:** [PENDING]

### `cargo test`
- **Result:** [PENDING]
- **Wall time:** [PENDING]
- **Failing tests:** [PENDING]
- **Ignored tests:** [PENDING]

### `cargo clippy --all-targets -- -D warnings`
- **Result:** Passed (Zero Errors, Zero Warnings)
- **Wall time:** 5.03s
- **Warning counts:** 0

### `lake build`
- **`lean/` Result:** Passed (0 sorry, 0 unproven axioms)
- **`proof/` Result:** Passed (0 sorry, 0 unproven axioms)

## 2. Invariant Shortcuts Count

The following is an empirical audit of `.unwrap()`, `.expect(`, `panic!`, and `unreachable!` by directory in `src/`:

| Directory | unwrap | expect | panic | unreachable | Total | Prior Baseline |
|-----------|:---:|:---:|:---:|:---:|:---:|:---:|
| `ast` | 1 | 0 | 0 | 0 | 1 | 1 |
| `codegen` | 3 | 0 | 0 | 3 | 6 | 143 |
| `ir` | 1 | 3 | 0 | 0 | 4 | 4 |
| `mir` | **0** | **0** | **0** | **0** | **0** | 14 |
| `opt` | 9 | 0 | 0 | 0 | 9 | 9 |
| `parser` | 13 | 0 | 0 | 4 | 17 | 17 |
| `runtime` | 1 | 0 | 0 | 0 | 1 | 1 |
| `testing` | 0 | 10 | 3 | 0 | 13 | 13 |
| `typecheck` | 6 | 1 | 0 | 1 | 8 | 8 |
| **Total** | **34** | **14** | **3** | **8** | **59** | **210** |

*Note: `mir` (lowering, symbolic execution, and supercompilation engine) achieves complete zero-panic and zero-unwrap purity.*
