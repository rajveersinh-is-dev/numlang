# Phase 63: True Production Self-Applicable Specializer (2nd Futamura Binary Output) — Plan

> **Phase**: 63
> **Status**: Complete
> **Traceability**: Master Plan Part VI, Requirements PROD-FUTA2-01..05
> **Milestone**: World's Fastest General Supercompiler (Phases 59–66)

## Objective
Enable `MinSpec.nl` to produce a runnable residual specializer executable binary when specialized on itself with program AST input, achieving a genuine 2nd Futamura projection that generates an autonomous native compiler binary.

## Root Cause / Motivation
While Phase 25/43 demonstrated Futamura equations structurally, producing a runnable native executable that functions as an independent, standalone compiler binary is the crowning achievement in supercompilation literature. This eliminates any claim that NumLang's Futamura capabilities are synthetic or non-executable.

## Requirements
- **PROD-FUTA2-01**: Upgrade `MinSpec.nl` to accept serialized AST byte streams and evaluate arbitrary NumLang programs.
- **PROD-FUTA2-02**: Drive `MinSpec.nl` specialized against itself using the supercompiler pipeline.
- **PROD-FUTA2-03**: Compile the resulting residual MIR into a native standalone executable binary (`target/release/minspec_cogen.exe`).
- **PROD-FUTA2-04**: Add `--futamura2` CLI command executing the self-specialization and verifying the generated binary.
- **PROD-FUTA2-05**: Verification in `tests/futamura2_binary_tests.rs`: the generated compiler binary compiles 10 distinct NumLang test programs, matching outputs of the primary compiler.

## Key Deliverables
- `src/stdlib/minspec.nl`, `src/compiler.rs`, `src/main.rs`
- Test suite: `tests/futamura2_binary_tests.rs`

## Verification Gate
- Generated executable compiles input programs into correct machine binaries.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
