# NumLang Benchmark Contract (v1.0)

**Frozen measurement contract for all empirical NumLang supercompiler performance claims.**
This document is the authoritative specification of how benchmark numbers in this repository
are produced, compared, validated, and published. All scripts (`scripts/make_bench_table.py`,
`scripts/ci_regression_guard.py`), documentation, and CI gates MUST conform to this contract.

---

## 1. Scope and Claim Boundaries

- The only sanctioned basis for a **"world's fastest supercompiler"** claim is:
  1. This contract being satisfied end-to-end for the frozen workload set,
  2. No folded cells being counted as numeric wins,
  3. The strict win rule being satisfied with 95% confidence,
  4. Compile-time and code-size metrics being measured and disclosed.
- A "world's fastest supercompiler" claim is **scoped to this contract**: i.e., "fastest on the
  frozen NumLang canonical literature benchmark set under the frozen flags, allocator policy,
  and statistical protocol on disclosed hardware" — nothing broader.

## 2. Workload Set

The frozen canonical literature benchmark set consists of **18 benchmarks** across three groups,
as registered in `bench/showdown/benchmarks.toml` and executed by `tests/supercompiler_showdown.rs`:

- **Group 1 — Classical Functional Deforestation (Wadler 1990, Hamilton 2007)**: `nrev` (Naive
  Reverse / Double nrev), `append3` (Triple List Append), `kmp` (Knuth-Morris-Pratt DFA),
  `peano_mul` (Peano Multiplication), `tree_flip` (Double Tree Inversion).
- **Group 2 — Arithmetic Recurrences**: `fib_matrix` (Coupled Fibonacci Matrix Power),
  `tri_sum` (Triangular Summation, 50M iterations), `cubic_sum` (Cubic Polynomial Sum, 10M),
  `pow2_mod` (Geometric Power Loop, 100), `hofstadter` (Hofstadter Mutual Linear Recurrence).
- **Group 3 — Higher-Order & Codata**: `compose5` (5-Deep Function Composition Chain),
  `map_map` (Map-Map Pipeline Deforestation), `sum_map` (Sum-Map Stream Fusion, 1M),
  `stream_take` (Stream Pipeline Filter-Sum).

Input sizes are those recorded in the canonical benchmark registry / sources
(e.g., `tri_sum` accumulates 50,000,000 iterations; `cubic_sum` evaluates 10,000,000 terms;
`sum_map` fuses 1,000,000 elements; `stream_take` runs 1000 repetitions of a 50-stream pipeline).

## 3. Compiler Flags and Equalization

- NumLang-SC and NumLang-Base runtime comparisons use the Cranelift backend via
  `numlang build --bench [--supercompile | --use-mir]`.
- Competitor runtime cells: Rust `rustc -C opt-level=3` (`Rustc-O3`); MSVC `cl.exe /O2 /nologo`
  (`MSVC-O2`); GHC `ghc -O2` (`GHC-O2`).
- **Equalization policy**: any cross-compiler runtime comparison must use equalized optimization
  levels going forward; the currently recorded disparity between `Rustc-O3` (`-O3`) and `MSVC-O2`
  (`/O2`) is disclosed as an equalization deviation and must be resolved for new claims.

## 4. Allocator Policy

- NumLang-SC and NumLang-Base share the identical Cranelift runtime allocator backend
  (`src/runtime/arena.c`).
- MSVC comparisons use static bump arrays; allocator-attribution experiments live in
  `docs/evidence/round2/allocator_attribution.csv` (R6).

## 5. Timing Protocol

- **Warmup**: >= 5 discarded warmup rounds.
- **Measured rounds**: >= 30 measured rounds per benchmark per system (quick mode: 2 + 5).
- **Clock**: in-process hardware performance counters (`QueryPerformanceCounter` on Windows,
  `clock_gettime(CLOCK_MONOTONIC)` on POSIX).
- **Folded threshold**: cells whose median runtime is <= 500 ns are treated as folded/invalid
  and disclosed as `<= 500 ns*` — never as numeric wins.

## 6. Statistical Decision Rule (Strict Win Rule)

A NumLang-SC benchmark is a **strict supercompiler win** iff the 95% confidence interval of the
Base/SC runtime ratio excludes 1.0 AND the effect is >= 5%:

- `ratio_low  > 1.05` → **NumLang-SC Win** (SC faster than Base by >= 5% with CI excluding 1.0).
- `ratio_high < 0.95` → **NumLang-Base Win** (SC slower than Base by >= 5% with CI excluding 1.0;
  a real residual regression).
- Otherwise → **Tie** (effect within noise threshold).

Ties and folded cells are never counted as fabricated wins.

## 7. Published Metrics

- **Residual runtime**: geometric mean of measured rounds per cell (exp of mean of logs).
- **Compile-time**: end-to-end compiler wall-clock time (ms) for the supercompilation build;
  **absent from `bench/data/showdown_results.csv` at contract v1.0 → published as `N/A`** until
  measured and recorded.
- **Code size**: emitted binary size in bytes (from the compile step); **absent from
  `bench/data/showdown_results.csv` at contract v1.0 → published as `N/A`** until measured and
  recorded.

## 8. Hardware and Toolchain Disclosure

Every published claim must disclose: CPU and OS of the measurement host, compiler versions
(`rustc`, MSVC `cl.exe`, GHC), detected-but-uninstalled competitors (GHC, HOSC, SPSC, Clang, GCC),
performance-counter implementation, and the 500 ns quantization floor.

## 9. Validation and CI Enforcement

- `scripts/make_bench_table.py --check` validates that every published number is traceable to a
  raw CSV measurement cell and that no fabricated numbers are emitted (`N/A` markers otherwise).
- `scripts/ci_regression_guard.py` fails CI on any real residual regression beyond the strict win
  rule, and tolerates folded cells.

---

**Contract version**: 1.0 — **Status**: Active (Phase 1 measurement foundation).
