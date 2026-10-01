---
phase: 41-decouple-win32-and-true-posix-native-codegen
plan: 41-01
subsystem: codegen
tags:
  - cross-platform
  - posix
  - win32-decoupling
  - cranelift
  - llvm
provides:
  - Isolated Win32 API declarations behind target_os = windows in Cranelift and LLVM code generators
  - Implemented standard POSIX libc system calls (exit, write, malloc) for non-Windows platforms
  - Updated entry_bench.c with ISO C exit and POSIX clock_gettime high-resolution timing
  - Updated link_unix in linker.rs with automatic entry_bench.c compilation for benchmark mode
  - Added OS-adaptive executable extension and compiler dispatch in runner.py and C benchmarks
affects:
  - 42-constructive-lean-4-soundness-proofs
  - 44-scoped-arena-allocator-and-memory-safety
  - 45-architecture-decomposition-codegen-unification-and-hardening
actuals:
  tasks: 4
  commits: 1
tech-stack:
  added: []
  patterns:
    - Conditional compilation via cfg(target_os = "windows") vs cfg(not(target_os = "windows"))
    - Dynamic host LLVM target triple detection via TargetMachine::get_default_triple()
    - Dual Windows QPC and POSIX clock_gettime hardware performance counter timing
key-files:
  created:
    - tests/platform_portability_tests.rs
    - .planning/phases/41-decouple-win32-and-true-posix-native-codegen/41-01-SUMMARY.md
  modified:
    - src/codegen/cranelift_backend.rs
    - src/codegen/llvm_backend.rs
    - src/codegen/entry_bench.c
    - src/codegen/linker.rs
    - src/main.rs
    - bench/harness/runner.py
    - bench/c/*.c
    - tests/multi_language_benchmarks.rs
    - tests/supercompiler_head_to_head.rs
key-decisions:
  - "Decoupled Windows kernel32 symbols ExitProcess, GetStdHandle, WriteFile into target_os = windows; declared libc exit and write for POSIX targets"
  - "Configured LLVM backend to retrieve host default target triple instead of hardcoding x86_64-pc-windows-msvc"
  - "Standardized entry_bench.c using C preprocessor guards to provide mainCRTStartup on Windows and standard main on POSIX with clock_gettime"
duration: 15min
completed: 2026-10-01
status: complete
---

# Phase 41 Summary: Decouple Win32 & True POSIX Native Codegen

## Completed Tasks

1. **Abstract System Calls in Cranelift & LLVM Codegen (`PORT-01`)**:
   - In `src/codegen/cranelift_backend.rs`, isolated `ExitProcess`, `GetStdHandle`, and `WriteFile` behind `#[cfg(target_os = "windows")]`. Declared POSIX libc `exit` and `write` on `#[cfg(not(target_os = "windows"))]`.
   - Updated `emit_helper_print_str`, `emit_panic`, and entry wrappers to use POSIX `write(1, ...)`, `write(2, ...)`, and `exit(code)` when compiled for non-Windows platforms.
   - In `src/codegen/llvm_backend.rs`, replaced hardcoded `"x86_64-pc-windows-msvc"` target triple with `TargetMachine::get_default_triple()`, declared libc `exit` and `write` for POSIX, and guarded Windows `mainCRTStartup` emission.

2. **Modernize `entry_bench.c` with ISO C and POSIX `clock_gettime` (`PORT-02`)**:
   - Refactored `src/codegen/entry_bench.c` to support both Windows and POSIX via `#ifdef _WIN32 ... #else ... #endif`.
   - Used `clock_gettime(CLOCK_MONOTONIC, &ts)` and standard ISO C `exit((int)ret);` on POSIX, retaining high-resolution QPC and `ExitProcess` on Windows.
   - Emitted standard C `main` calling `numlang_main` on POSIX.

3. **Update Linker, Runner, and Benchmark Baselines (`PORT-03`)**:
   - In `src/codegen/linker.rs`, updated `link_unix` to automatically compile embedded `entry_bench.c` when `NUMLANG_BENCH` is set.
   - In `src/main.rs`, defaulted output executable extension to `.exe` on Windows and empty string `""` on POSIX.
   - In `bench/harness/runner.py`, adapted binary naming (`numlang` on Linux vs `numlang.exe` on Windows), process affinity pinning (`os.sched_setaffinity`), and added `-lm` linking with `clang`/`gcc`.
   - In `bench/c/*.c` (all 13 C benchmarks), added `#ifdef _WIN32` guards and POSIX `clock_gettime` fallbacks.
   - In `tests/multi_language_benchmarks.rs` and `tests/supercompiler_head_to_head.rs`, updated `wrap_c` to support POSIX compilation.

4. **Cross-Platform Verification & Dockerfile Audit (`PORT-04`)**:
   - Created `tests/platform_portability_tests.rs` with 6 regression tests verifying platform decoupling, linker options, and symbol isolation.
   - All tests pass with zero failures and 0 clippy warnings under `cargo clippy --all-targets -- -D warnings`.
