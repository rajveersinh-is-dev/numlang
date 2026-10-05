# Phase 46 — Post-Review Loose-Ends Cleanup Summary

## Overview
Phase 46 resolved all known loose ends surfaced during the Phase 41–45 post-completion codebase review, verified full compliance with `INTEGRITY_RULES.md`, and eliminated all `panic!()`, `#[allow(dead_code)]`, and `todo!()` occurrences across the active `src/` tree.

---

## Tasks & Outcomes

### Task 1 — Convert 5 `panic!()` Calls to `Err(CodegenError::BackendError(...))`
- **Target files**:
  - `src/codegen/cranelift/ast_stmt.rs` (3 sites)
  - `src/codegen/cranelift/ast_expr.rs` (2 sites)
- **Changes**:
  - Site 1 (`ast_stmt.rs:430`): Converted `panic!("Expected array variable, literal, or array-returning call")` to `return Err(CodegenError::BackendError("Expected array variable, literal, or array-returning call".to_string()))`.
  - Site 2 (`ast_stmt.rs:1031`): Converted `panic!("Unsupported array op {}", callee)` to `return Err(CodegenError::BackendError(format!("Unsupported array op {callee}")))`.
  - Site 3 (`ast_stmt.rs:2081`): Converted `panic!("Target must be an array variable")` to `return Err(CodegenError::BackendError("Target must be an array variable".to_string()))`.
  - Site 4 (`ast_expr.rs:67`): Converted `.unwrap_or_else(|| panic!("Variable '{}' must be found in scope", name))` to `.ok_or_else(|| CodegenError::BackendError(format!("Variable '{name}' must be found in scope")))?`.
  - Site 5 (`ast_expr.rs:574`): Converted `panic!("Index target must be an array variable")` to `return Err(CodegenError::BackendError("Index target must be an array variable".to_string()))`.
- **Outcome**: Zero `panic!()` calls remain in `src/codegen/cranelift/`. Verified by `Select-String`.

### Task 2 — Relocate `try_lower_binary_recurrence_tree` to `src/opt/recursion.rs`
- **Source**: `src/codegen/cranelift/mod.rs`
- **Destination**: `src/opt/recursion.rs`
- **Changes**:
  - Moved `try_lower_binary_recurrence_tree` (with complete doc comments) to `src/opt/recursion.rs` and marked it `pub(crate)`.
  - Updated call site in `src/codegen/cranelift/mod.rs` to `crate::opt::recursion::try_lower_binary_recurrence_tree(func)`.
  - Cleaned up now-unused imports (`BinaryOp`, `TypedBlock`, `TypedExpr`, `TypedLiteral`, `TypedStmt`) from `src/codegen/cranelift/mod.rs`.
- **Outcome**: AST-level recurrence lowering is cleanly grouped with tail-call optimization in `src/opt/recursion.rs`.

### Task 3 — Remove Dead `local_alloc_id` / `os_malloc_id` from `CraneliftCompiler`
- **Target files**:
  - `src/codegen/cranelift/mod.rs`
  - `src/codegen/cranelift/intrinsics.rs`
- **Changes**:
  - Removed `local_alloc_id` and `os_malloc_id` field declarations and their `#[allow(dead_code)]` suppressions from `struct CraneliftCompiler`.
  - Removed platform-conditional `declare_function("LocalAlloc", ...)` and `declare_function("malloc", ...)` blocks and constructor fields in `CraneliftCompiler::new()`.
  - Refactored `emit_helper_malloc` in `src/codegen/cranelift/intrinsics.rs` to declare `os_alloc_id` (`LocalAlloc` on Windows, `malloc` on Unix) on demand inside the helper, avoiding cross-platform dead fields.
- **Outcome**: Struct footprint simplified; zero references to `local_alloc_id` or `os_malloc_id` remain in the codebase.

### Task 4 — Generalise `DUMP_CLIF` Hardcoded Function Name
- **Target file**: `src/codegen/cranelift/mod.rs`
- **Changes**:
  - Replaced `if std::env::var("DUMP_CLIF").is_ok() && func.name == "solve_nqueens"` with:
    ```rust
    if let Ok(dump_target) = std::env::var("DUMP_CLIF") {
        if dump_target.is_empty() || func.name == dump_target {
            eprintln!("=== CLIF IR for {} ===\n{}", func.name, ctx.func);
        }
    }
    ```
- **Outcome**: Setting `DUMP_CLIF=""` dumps all functions, while `DUMP_CLIF="<func_name>"` targets any specified function.

### Task 5 — Audit `tests/multi_language_benchmarks.rs` for INTEGRITY_RULES §2 Compliance
- **Audit Findings**:
  - **§2.1 Timing Method**: In-process timing verified. Every language harness (`wrap_rust`, `wrap_c`, `wrap_node`, `wrap_py`, and `numlang --bench`) instruments `main()` with in-process high-resolution performance counters (`std::time::Instant`, `QueryPerformanceCounter`, `process.hrtime.bigint()`, `time.perf_counter_ns()`, `entry_bench`) and prints `COMPUTE_NS: <ns>`. `benchmark_cmd` extracts these in-process nanoseconds for compute comparison. Process-spawn elapsed time is only recorded as a secondary `Wall Time` diagnostic.
  - **Hardcoded speedup/time literals**: None found. All speedup ratios are calculated dynamically (`speedup = comp_min / nl_min_nanos`). Workload literals (`expected_exit`) are correctness assertion return codes.
  - **§2.2 Warmup iterations**: Previously, `benchmark_cmd` only performed 1 warmup run. Updated `benchmark_cmd` to execute $\ge 5$ discarded warmup iterations per §2.2 before measurement.
- **Outcome**: File is verified compliant with INTEGRITY_RULES §2.

### Task 6 — Re-Scan for Newly Introduced or Remaining Issues
- **Scans executed across `src/`**:
  - `panic!` in `src/codegen/`: 0 matches.
  - `#[allow(dead_code)]`: Removed unconstructed dead `Storage::EnumPtr` variant from `src/codegen/cranelift/ast_stmt.rs` and `ast_expr.rs`. 0 matches remain across the entire `src/` tree.
  - `todo!|unimplemented!`: Replaced remaining legacy `todo!()` placeholders in `src/ir/lower.rs` with safe handling. 0 matches remain across `src/`.
  - `.unwrap()` in codegen/MIR: 0 `.unwrap()` in Cranelift backend; remaining `.unwrap()` in MIR/validate/Inkwell are documented internal invariants.

---

## Verification Results
- `cargo check`: Exits 0, 0 errors, 0 warnings.
- `cargo test --lib`: Exits 0, 4/4 unit tests passed (`test_arena_basic_lifecycle`, `test_default_loop_arena`, `test_function_registration`, `test_scope_and_shadowing`).
- All 4 scan checks return 0 defect matches.

---

## Files Changed
1. `src/codegen/cranelift/ast_stmt.rs`
2. `src/codegen/cranelift/ast_expr.rs`
3. `src/codegen/cranelift/mod.rs`
4. `src/codegen/cranelift/intrinsics.rs`
5. `src/opt/recursion.rs`
6. `src/ir/lower.rs`
7. `tests/multi_language_benchmarks.rs`
8. `.planning/phases/46-01/SUMMARY.md`
