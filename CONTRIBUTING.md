# Contributing to NumLang

Thank you for contributing to NumLang. NumLang is an experimental research programming language and supercompiler designed under strict computational honesty rules ([`INTEGRITY_RULES.md`](INTEGRITY_RULES.md)).

## Prerequisites

- **Rust**: Stable toolchain (1.82+) with `rustfmt` and `clippy`.
- **Lean 4**: Install via `elan` (`lake build` inside `lean/`).
- **Python**: 3.10+ for running benchmark harnesses and reproducibility checks.
- **Clang / LLD**: Optional for standalone LLVM object file generation and native linking.

## Building & Testing

```bash
# Build compiler binary
cargo build --release

# Run full test suite
cargo test

# Check formatting and style
cargo fmt -- --check

# Enforce zero clippy warnings
cargo clippy --all-targets -- -D warnings
```

## Formal Proof Verification

To verify formal semantics and supercompiler preservation theorems in Lean 4:

```bash
cd lean
lake build
```

Ensure no `sorry` or unproven `axiom` declarations are introduced into proofs.

## Benchmarks & Profiling

```bash
# Run benchmark smoke test suite
cargo test --test structural_generality_tests
cargo test --test general_recurrence_tests

# Run in-process microsecond hardware benchmark suite
python bench/harness/runner.py
```

## Integrity Guidelines

1. **Zero Precalculated Answers**: Never hardcode answers, lookup tables, or match user function names.
2. **Computational Honesty**: Timings must be measured in-process over $\ge 5$ warmup runs and $\ge 30$ measurement iterations.
3. **Purity**: Zero `panic!()` or `.unwrap()` in production lowering and codegen pipelines.
