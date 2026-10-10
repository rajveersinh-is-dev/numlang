# Phase 4 Self-Audit: Honest, Fair Benchmarking

Date: 2026-10-10  
Auditor / Orchestrator: Team Peer Review  
Status: **DONE**  

---

## 1. Hostile Re-read: The 5 Weakest Points & Attack Experiments

1. **Weak Point 1: Unequal Competitor Flags & Rustc Optimization Disparity (F3)**
   - *Attack*: Inspected invocation flags for competitors in `tests/supercompiler_showdown.rs`. Rust was compiled with `-O` (`-C opt-level=2`) while MSVC used `/O2`.
   - *Result*: Rust was subjected to lower optimization levels than MSVC.
   - *Resolution*: Upgraded Rust invocation in `compile_rust` to `-C opt-level=3` (`Rustc-O3`). Re-verified detection and compilation.

2. **Weak Point 2: Sub-Timer Floor Quantization & Fabricated Wins (F4, F7)**
   - *Attack*: Benchmarks where runtimes dropped to $0$–$100$ ns (`tri_sum`, `cubic_sum`, `pow2_mod`, `compose5`).
   - *Result*: Previously, microbenchmarks evaluating at compile-time closed-forms were claimed as "wins" for NumLang-SC over Rustc even when both evaluated instantaneously below the hardware performance counter quantization floor ($\le 500$ ns).
   - *Resolution*: Implemented transparent floor detection in `format_status` (`≤ 500 ns*`) and winner assignment (`Tie (≤500ns)*`). Only true, above-floor speedup wins are tallies as NumLang-SC outright wins. Outright wins are 5/14 (35.7%), floor ties are 4/14 (28.6%), and combined parity is 9/14 (64.3%).

3. **Weak Point 3: AST Codegen vs MIR Codegen Baseline Inequity (F1)**
   - *Attack*: Compared `NumLang-SC` against `NumLang-Base`.
   - *Result*: `NumLang-Base` previously invoked AST Cranelift codegen (`compile_to_obj_with_opt`) while `NumLang-SC` used the MIR Cranelift backend (`compile_mir_to_obj`). Comparing two distinct backend pipelines masked the true effect of the supercompiler.
   - *Resolution*: Updated `compile_numlang` to pass `--use-mir` for the baseline. The comparison is now strictly apples-to-apples on the MIR Cranelift backend, showing that supercompilation matches or accelerates baseline MIR execution across all deforestation benchmarks (e.g. `nrev`: 248.2µs vs 265.7µs, 1.07x speedup).

4. **Weak Point 4: Algorithm Naming & Modulo Claims (F5)**
   - *Attack*: Audited benchmark descriptions for `tri_sum` and `cubic_sum`.
   - *Result*: Documentation claimed $\pmod{10^9+7}$ reductions, whereas the actual `.nl` benchmarks evaluated pure sums with `% 256` exit code truncations. Additionally, `cubic_sum` evaluated the sum of squares ($\sum i^2$).
   - *Resolution*: Corrected algorithm descriptions in `BenchmarkSpec` and `SHOWDOWN.md`. `cubic_sum` is explicitly labeled "Sum of Squares 1^2+...+10M^2 (Degree-3 Faulhaber closed form)", and spurious $\pmod{10^9+7}$ claims were removed.

5. **Weak Point 5: Reproducibility with Discarded Warmups & In-Process Monotonic Timers**
   - *Attack*: Audited measurement loop in `measure_binary`.
   - *Result*: Confirmed strict compliance with Rule G2: 5 discarded warmup executions, followed by 30 high-resolution in-process monotonic timing rounds per competitor per benchmark, with correctness gates on exit codes before any timing measurement.

---

## 2. Verification Evidence

- `cargo test --test supercompiler_showdown -- --nocapture`: **PASSED** (14 benchmarks, 30 rounds + 5 warmups, all exit codes verified)
- `SHOWDOWN.md`: Synchronized with live measured data and honest tie classifications
- `bench/data/showdown_results_sample.csv`: Regenerated with fresh measurements
- `python scripts/claims_lint.py`: **PASSED** (100% claim integrity satisfied)
- `cargo clippy --all-targets -- -D warnings`: **PASSED** (0 errors, 0 warnings)

---

## 3. Assumptions, Guesses & Unverified Areas

- Competing compilers not available on Windows (GHC, HOSC, SPSC, Clang, GCC) are marked `NOT_INSTALLED`. Full multi-platform comparison with GHC is executed on Ubuntu CI runners.

---

## 4. Confidence Assessment

- **Benchmark Fairness**: **HIGH**. Full flags parity (`-C opt-level=3`, `/O2 /nologo`), apples-to-apples MIR baseline.
- **Timer Floor Integrity**: **HIGH**. Sub-500ns measurements classified as ties rather than false numeric wins.
- **Data Provenance**: **HIGH**. Measured in-process on real hardware with full raw round logs emitted to CSV.
