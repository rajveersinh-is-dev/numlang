---
phase: "41"
name: "decouple-win32-and-true-posix-native-codegen"
created: 2026-10-01
status: passed
---

# Phase 41: decouple-win32-and-true-posix-native-codegen — Verification

## Goal-Backward Verification

**Phase Goal:** Decouple hardcoded Windows kernel32 symbols (`ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`) from native code generation; support standard POSIX libc system calls (`exit`, `write`, `malloc`) on non-Windows platforms; fix C benchmark wrappers and arithmetic safety.

## Checks

| # | Requirement | Status | Evidence |
|---|------------|:------:|----------|
| 1 | Cranelift Syscall Abstraction | Passed | `src/codegen/cranelift_backend.rs` conditionally declares `ExitProcess`/`GetStdHandle`/`WriteFile` on Windows, and `exit`/`write`/`malloc` on POSIX. |
| 2 | LLVM Backend Portability | Passed | `src/codegen/llvm_backend.rs` targets host triple dynamically and conditionally declares `exit`/`write` on non-Windows. |
| 3 | Portable Benchmark Entry C Wrapper | Passed | `src/codegen/entry_bench.c` implements `#ifdef _WIN32` / `#else` with `clock_gettime(CLOCK_MONOTONIC)` and standard `exit()`. |
| 4 | Unix Benchmark Linking Support | Passed | `src/codegen/linker.rs` embeds `ENTRY_BENCH_C` and invokes `cc` with temporary `entry_bench.c`, `-lm`, and `-no-pie`. |
| 5 | Order-2 Recurrence Arithmetic Safety | Passed | `src/mir/supercompiler/generalize.rs` uses `checked_mul`, `checked_add`, `is_some_and`, and non-zero divisor guard (`den_a != 0`). |
| 6 | Cross-Platform Integration Test Suite | Passed | `tests/platform_portability_tests.rs` (6/6 passing). |
| 7 | Full Regression Test Suite | Passed | `cargo test --tests` (74 suites, 100% green). |
| 8 | Clippy Hygiene | Passed | `cargo clippy --all-targets -- -D warnings` (0 warnings). |

## Result

**Phase 41 PASSED**. All Win32 dependencies have been decoupled into conditional abstractions with full POSIX support.
