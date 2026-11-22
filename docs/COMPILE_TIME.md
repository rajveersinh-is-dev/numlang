# Compile-Time Benchmark Methodology

This document summarizes the compile-time benchmark harness (`tests/compile_time_benchmarks.rs`) and the measured data in `bench/data/compile_times.csv`.

## Methodology

The harness measures **supercompiler specialization latency** — the in-process time for the NumLang supercompiler to specialize a typed AST through distillation + supercompilation, MIR lowering, and `supercompile_mir_program`. Two modes are supported:

| Mode | Warmups (discarded) | Measured rounds | Invocation |
|------|---------------------|-----------------|------------|
| **FULL** (default) | 5 | 30 | `cargo test --test compile_time_benchmarks` |
| **QUICK** | 2 | 5 | `COMPILE_TIME_QUICK=1` or `QUICK_BENCHMARKS=1` |

Timing uses `std::time::Instant` only: `QueryPerformanceCounter` on Windows and `clock_gettime(CLOCK_MONOTONIC)` on POSIX. No wall-clock `SystemTime`, no lookup tables, and no synthetic values. Each typed AST is cloned once per round outside the timed region, and each residual `MirProgram` is passed through `std::hint::black_box` to prevent elimination.

The CSV schema (`bench/data/compile_times.csv`):

```
benchmark_id,system,warmup_ns,median_compile_ns,p95_compile_ns,rounds,status
```

- `warmup_ns` is the **summed** duration of the discarded warmup rounds.
- `median_compile_ns` and `p95_compile_ns` use nearest-rank percentiles over the measured rounds, consistent with `tests/supercompiler_showdown.rs`.
- `rounds` records the number actually used.
- Missing or unreadable sources produce `SKIPPED_*` rows and never fail the test.

## Measured Data

The committed CSV contains 36 rows: 18 workloads (14 canonical microbenchmarks plus 4 extended realistic workloads) × 2 systems (Base + SC).

Below are the median and p95 compile times in nanoseconds, exactly as written to the CSV:

| Benchmark | System | warmup_ns | median_compile_ns | p95_compile_ns | rounds |
|-----------|--------|-----------|-------------------|----------------|--------|
| nrev | NumLang-SC-specialize | 48,037,500 | 9,240,600 | 14,960,500 | 30 |
| nrev | NumLang-Base-lower | 3,246,300 | 632,500 | 721,200 | 30 |
| append3 | NumLang-SC-specialize | 72,285,300 | 14,084,900 | 20,668,600 | 30 |
| append3 | NumLang-Base-lower | 6,121,100 | 1,001,900 | 1,129,100 | 30 |
| kmp | NumLang-SC-specialize | 54,659,200 | 10,555,300 | 11,459,300 | 30 |
| kmp | NumLang-Base-lower | 2,959,200 | 554,800 | 610,100 | 30 |
| peano_mul | NumLang-SC-specialize | 41,700,200 | 7,912,000 | 8,647,100 | 30 |
| peano_mul | NumLang-Base-lower | 3,568,500 | 668,800 | 697,000 | 30 |
| tree_flip | NumLang-SC-specialize | 577,840,700 | 104,592,200 | 129,943,600 | 30 |
| tree_flip | NumLang-Base-lower | 3,053,800 | 583,300 | 627,000 | 30 |
| fib_matrix | NumLang-SC-specialize | 410,756,900 | 84,087,100 | 105,808,600 | 30 |
| fib_matrix | NumLang-Base-lower | 1,297,500 | 237,300 | 284,000 | 30 |
| tri_sum | NumLang-SC-specialize | 8,237,200 | 1,569,500 | 1,703,500 | 30 |
| tri_sum | NumLang-Base-lower | 1,202,900 | 230,200 | 253,600 | 30 |
| cubic_sum | NumLang-SC-specialize | 8,807,800 | 1,710,900 | 1,837,200 | 30 |
| cubic_sum | NumLang-Base-lower | 1,236,400 | 239,700 | 257,300 | 30 |
| pow2_mod | NumLang-SC-specialize | 9,433,500 | 1,818,600 | 3,110,700 | 30 |
| pow2_mod | NumLang-Base-lower | 1,160,600 | 222,600 | 277,000 | 30 |
| hofstadter | NumLang-SC-specialize | 4,808,697,900 | 972,449,500 | 1,427,566,400 | 30 |
| hofstadter | NumLang-Base-lower | 5,096,600 | 1,011,200 | 2,085,500 | 30 |
| compose5 | NumLang-SC-specialize | 16,322,800 | 3,183,900 | 5,106,200 | 30 |
| compose5 | NumLang-Base-lower | 4,009,800 | 487,900 | 1,083,000 | 30 |
| map_map | NumLang-SC-specialize | 551,119,200 | 89,033,400 | 117,512,400 | 30 |
| map_map | NumLang-Base-lower | 1,405,100 | 224,700 | 254,600 | 30 |
| sum_map | NumLang-SC-specialize | 5,962,900 | 1,093,900 | 1,169,800 | 30 |
| sum_map | NumLang-Base-lower | 749,500 | 141,000 | 148,100 | 30 |
| stream_take | NumLang-SC-specialize | 1,323,667,900 | 248,013,800 | 270,607,500 | 30 |
| stream_take | NumLang-Base-lower | 1,642,500 | 293,500 | 303,000 | 30 |
| expr_eval | NumLang-SC-specialize | 954,656,300 | 186,854,500 | 220,623,700 | 30 |
| expr_eval | NumLang-Base-lower | 3,085,300 | 582,900 | 627,700 | 30 |
| window_stats | NumLang-SC-specialize | 25,631,200 | 4,939,500 | 5,150,700 | 30 |
| window_stats | NumLang-Base-lower | 2,024,800 | 382,800 | 409,100 | 30 |
| record_pipeline | NumLang-SC-specialize | 24,289,900 | 4,756,600 | 4,983,800 | 30 |
| record_pipeline | NumLang-Base-lower | 10,818,300 | 2,067,800 | 2,303,900 | 30 |
| nested_stencil | NumLang-SC-specialize | 72,265,100 | 14,147,400 | 15,805,100 | 30 |
| nested_stencil | NumLang-Base-lower | 7,041,800 | 1,358,700 | 1,646,800 | 30 |

All 36 rows are present in the CSV.

## Median Summary (all 36 rows)

The overall median compile time across all benchmarks and systems is **1,464,100 ns** (≈1.5 ms). The SC-specialize median is **9,897,950 ns** (≈9.9 ms), vs Base median of **521,350 ns** (≈0.5 ms). On this host the SC/Base ratio ranges from **2.3×** (record_pipeline) to **961.7×** (hofstadter); the extreme end reflects supercompilation of deeply recursive functions where baseline lowering is nearly free. These numbers were measured after removing redundant MIR-block and ancestor-state clones from symbolic driving; see `bench/data/compile_times.csv` for the raw rows.

The committed CSV is the final clean post-fix run. Against the pre-fix baseline captured with the same harness, the sum of the 18 SC medians fell from **1,897,736,000 ns** to **1,760,043,600 ns** (**−7.3%**), the per-workload median change was **−10.4%**, and **16/18** workloads improved. Largest improvements were `kmp` (−68.3%), `peano_mul` (−67.6%), `append3` (−66.2%), and `nrev` (−65.1%). The only increases were `compose5` (+12.5%) and `sum_map` (+0.4%), both short noisy workloads.

Profile runs used `NUMLANG_PROFILE_SUPERCOMPILE=1 cargo test --test specialization_latency_tests profile_append3_and_expr_eval -- --nocapture`. With separate `CARGO_TARGET_DIR` values for baseline and fixed builds, medians over six paired runs show symbolic driving dropping from **5.944 ms → 5.853 ms** for `append3` and **173.313 ms → 147.424 ms** for `expr_eval` (**−14.9%**); total specialization time dropped from **183.569 ms → 158.140 ms** for `expr_eval`.

## Scope and Notes

- **This is specialization latency, not end-to-end wall-clock compile time.** It does not include process startup, linking, or binary emission. A future `numlang build` wall-clock column should be added separately rather than mislabeling these numbers as full build time.
- **NEW metric not yet reflected in the frozen contract tables.** The compile-time CSV and this methodology were added to close the contract v1.0 §7 gap where compile-time was previously `N/A`. The existing frozen tables (README.md, SHOWDOWN.md, BENCHMARK_CONTRACT.md) only track `expected_exit` and `COMPUTE_NS`; they do not include specialization latency columns.
- The four extended workloads (`expr_eval`, `window_stats`, `record_pipeline`, `nested_stencil`) are included in the 36-row CSV with SUCCESS status, measured on the same host: 11th Gen Intel(R) Core(TM) i5-11260H @ 2.60GHz, Windows 11 Home, Rust 1.100.0.
- Re-run paired comparisons with separate target directories: sharing one `CARGO_TARGET_DIR` between different source trees can reuse a stale test binary and invalidate the comparison.
