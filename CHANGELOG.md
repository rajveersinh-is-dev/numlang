# Changelog

All notable changes to the NumLang compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Generalized order-2 linear recurrence emitter `__numlang_linear_rec2` supporting arbitrary constant-coefficient recurrences ($s_{k+1} = c_1 s_k + c_2 s_{k-1}$) with symmetrical Cranelift and LLVM lowering.
- System `clang` CLI fallback in `LlvmCompiler` for emitting `.obj` native objects when built without the `inkwell` feature.
- Multi-threaded parallel test runner for differential fuzzing tests (`test_differential_10k`) with adaptive debug/release tiering.
- Automated monograph PDF verification script (`paper/book/audit_pdf.py`).
- Pre-commit integrity lint test (`tests/integrity_lint.rs`) blocking string comparisons on benchmark and function names.

### Changed
- Eradicated all 41 production invariant shortcuts (`unwrap`, `expect`, `unreachable`) outside tests across the parser, typechecker, IR lowering, and codegen backends.
- Converted all heuristic `__numlang_fib` pattern matches to inductive algebraic linear recurrence representations.
- Dynamically scaled differential validation test counts in debug mode for rapid CI turnaround while preserving 10,000 cases in release mode.
- Replaced static binary `src/codegen/entry_bench.obj` with dynamic on-demand compilation from `entry_bench.c`.

### Fixed
- Fixed parser error propagation on invalid token sequences and missing block delimiters.
- Fixed field index lookup error handling in struct typechecking.
- Hardened deoptimization table lock acquisition against poisoned locks.
- Synchronized package metadata and licensing in `Cargo.toml` and `LICENSE`.
