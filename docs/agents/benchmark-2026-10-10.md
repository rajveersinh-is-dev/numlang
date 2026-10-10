# Sub-Agent Report: Benchmark Agent

Date: 2026-10-10  
Role: Benchmark Agent  
Owned Files: `tests/supercompiler_showdown.rs`, `bench/data/showdown_results_sample.csv`, `SHOWDOWN.md`  

---

## 1. What Was Done
- Audited the multi-compiler showdown benchmark harness under Rule G2 and G4.
- Standardized compiler optimization flags: Rust compiles with `-C opt-level=3` (`Rustc-O3`), MSVC with `/O2 /nologo`.
- Aligned NumLang baseline to `--use-mir` for apples-to-apples comparison on identical Cranelift MIR backend.
- Handled hardware performance counter quantization floor ($\le 500$ ns): displayed as `≤ 500 ns*` and recorded as `Tie (≤500ns)*`, forbidding false win inflation.
- Executed 14 canonical literature benchmarks across 5 discarded warmups and 30 measurement rounds. Emitted raw measurement data to CSV.
- Updated `SHOWDOWN.md` with honest, unadulterated numbers.

## 2. What Was Verified (Provenance Tags)
- [RAN] `cargo test --test supercompiler_showdown -- --nocapture` (Passed, 14 benchmarks measured across 30 rounds)
- [READ] `bench/data/showdown_results_sample.csv` (Freshly written CSV logs)
- [READ] `SHOWDOWN.md` (Synchronized with CSV data)

## 3. What Could Not Be Verified
- GHC, HOSC, and SPSC are not installed on the Windows test host; marked `NOT_INSTALLED` and excluded from win tallies.

## 4. Assumptions Made
- In-process monotonic clock (`QueryPerformanceCounter` on Windows) has ~100ns quantization resolution. Runtimes $\le 500$ ns are treated as sub-timer floor.
