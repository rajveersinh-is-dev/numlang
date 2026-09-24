# Phase 2.4 Plan: CLI Tooling, Integration & Verification Gate

## Goal
Wire up user-facing CLI tooling (`--emit-memory-ssa`), construct a dedicated comprehensive integration test suite `tests/memory_ssa_tests.rs`, and pass the full verification gate (100% test pass rate, 0 Clippy warnings, release build).

## Tasks

1. **CLI Tooling (`src/main.rs`) (TEST-01)**:
   - Add `--emit-memory-ssa` CLI argument in `src/main.rs` (analogous to `--emit-mir`, `--emit-ast`, etc.).
   - When `--emit-memory-ssa` is passed:
     - Lex, parse, typecheck source.
     - Lower to MIR (`lower_program`).
     - Build `MemorySSA` for each function and print `mssa.display(func)`.
     - Exit cleanly with code 0.

2. **Integration Test Suite (`tests/memory_ssa_tests.rs`) (TEST-02)**:
   - Create end-to-end integration tests:
     - CLI execution testing `numlang --emit-memory-ssa <file.nl>`.
     - Multi-branch conditional MemorySSA verification.
     - Nested loop MemorySSA with IDF $\phi$-nodes.
     - Struct field sensitivity & promotion integration.
     - DSE/RLE end-to-end impact verification.

3. **Full Verification Gate (TEST-03)**:
   - Run `cargo test --tests` to ensure all 38 test suites pass without regressions.
   - Run `cargo clippy --all-targets -- -D warnings` to verify zero warnings.
   - Push completed commits to GitHub `origin master`.
