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
- **Result:** [PENDING]
- **Wall time:** [PENDING]
- **Warning counts:** [PENDING]

### `lake build`
- **`lean/` Result:** Passed
- **`proof/` Result:** Passed

## 2. Invariant Shortcuts Count

The following is a count of `.unwrap()`, `.expect(`, `panic!`, and `unreachable!` by directory in `src/`:

| Directory | Count |
|-----------|-------|
| `ast` | 1 |
| `codegen` | 143 |
| `ir` | 4 |
| `mir` | 14 |
| `opt` | 9 |
| `parser` | 17 |
| `runtime` | 1 |
| `testing` | 13 |
| `typecheck` | 8 |

*Note: `mir` contains lowering and supercompiler.*
