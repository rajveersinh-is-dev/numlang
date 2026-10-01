---
phase: "41"
name: "decouple-win32-and-true-posix-native-codegen"
created: 2026-10-01
status: passed
---

# Phase 41: Decouple Win32 & True POSIX Native Codegen — Verification

## Goal-Backward Verification

**Phase Goal:** Enable clean, native Linux/macOS compilation and ensure the Docker reproduction container executes without unresolved Windows kernel32 symbols.

## Checks

| # | Requirement | Status | Evidence |
|---|------------|--------|----------|
| 1 | PORT-01: Abstract runtime platform imports across native code generators | PASSED | `src/codegen/cranelift_backend.rs` and `src/codegen/llvm_backend.rs` isolate `ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc` behind `target_os = "windows"`, declaring libc `exit`, `write`, `malloc` for non-Windows. |
| 2 | PORT-02: Update `entry_bench.c` with standard ISO C `exit` and POSIX `clock_gettime` | PASSED | `src/codegen/entry_bench.c` supports `#ifdef _WIN32` and `#else` with `clock_gettime(CLOCK_MONOTONIC, &ts)` and `exit((int)ret);`. |
| 3 | PORT-03: Update `link_unix`, `runner.py`, and C benchmarks for POSIX compatibility | PASSED | `link_unix` compiles embedded `entry_bench.c` when `NUMLANG_BENCH` is set; `runner.py` adapts binary paths/suffixes; all 13 C benchmarks updated with `clock_gettime`. |
| 4 | PORT-04: Cross-platform verification & Dockerfile audit | PASSED | `tests/platform_portability_tests.rs` (6 tests) pass 100%; `cargo clippy --all-targets -- -D warnings` completes with 0 warnings. |

## Result

All checks passed with full automated test coverage and 0 compiler warnings. Phase 41 requirements `PORT-01` through `PORT-04` are satisfied.
