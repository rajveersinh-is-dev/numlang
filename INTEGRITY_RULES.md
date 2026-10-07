# Core Engineering & Computation Integrity Rules

## 1. Absolute Prohibition of Pre-Loaded Numbers & Hardcoded Shortcuts
- **NO PRELOADING OF NUMBERS**: Zero pre-loaded lookup tables, hardcoded recurrence outputs, precalculated benchmark answers, or synthetic constants anywhere in the codebase.
- **EVERYTHING MUST BE COMPUTED AS IS**: All computational values must be evaluated dynamically from first principles, algorithmic execution, or verified symbolic mathematical derivation.
- **NO BENCHMARK NAME COUPLING**: Never write optimization rules matching benchmark names (e.g. `ack`, `tak`, `fib`, `nrev`, `append3`) or string heuristics. All optimizations (recurrence detection, permutation matching, deforestation) must be purely structural, general, and inductive across all user-defined code.

## 2. Computational Honesty & Real Benchmarking
- Every benchmark and test must execute the full, unadulterated workload to completion.
- Microbenchmarks must be timed in-process using high-resolution hardware performance counters (`QueryPerformanceCounter` on Windows, `clock_gettime(CLOCK_MONOTONIC)` on POSIX, `Instant::now()` in Rust) across $\ge 30$ measurement rounds following $\ge 5$ discarded warmup iterations.
- Process exit codes must be strictly verified (`assert returncode == 0`); crashes must be logged transparently as `ERROR` / `CRASH`.
- Zero fake stubs: every optimization claimed (Hamilton distillation, Mitchell-Klyuchnikov MRSC, polyhedral stencil fusion, Reynolds defunctionalization, translation validation) must be a full, mathematically authentic implementation.

## 3. Strict Compiler Invariants & Clean Codebase
- **Zero Panic in Codegen**: Never use `panic!()` or `.unwrap()` in code generation and lowering pipelines; always use structured domain `CodegenError` types.
- **Zero Dead Code**: Never leave `#[allow(dead_code)]` or unused variables/fields in the active codebase.
- **Zero Compiler Warnings**: All code must build cleanly under `cargo clippy --all-targets -- -D warnings` and `cargo check --tests` with 0 errors and 0 warnings.
- **Zero Axiom / Sorry Shortcuts**: All formal verification proofs in Lean 4 must compile cleanly with 0 `sorry` and 0 unproven axioms.

## 4. Algorithmic Generality & Symmetric Optimization
- All compiler optimization and supercompilation passes must treat user code symmetrically and agnostically: identical transformations must fire regardless of function names, parameter identifiers, or source layout.
