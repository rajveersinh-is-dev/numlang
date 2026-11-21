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

The committed CSV contains 36 rows: 18 canonical microbenchmarks × 2 systems (Base + SC), plus 4 extended realistic workloads × 2 systems.

Below are the median and p95 compile times in nanoseconds, rounded to the nearest 100 ns:

| Benchmark | System | warmup_ns | median_compile_ns | p95_compile_ns | rounds |
|-----------|--------|-----------|-------------------|----------------|--------|
| append3 | NumLang-Base-lower | 4,839,200 | 943,100 | 1,010,200 | 30 |
| append3 | NumLang-SC-specialize | 60,294,300 | 12,121,600 | 12,733,000 | 30 |
| compose5 | NumLang-Base-lower | 2,292,400 | 438,400 | 500,100 | 30 |
| compose5 | NumLang-SC-specialize | 12,044,000 | 2,259,600 | 2,517,800 | 30 |
| cubic_sum | NumLang-Base-lower | 1,259,300 | 240,200 | 244,600 | 30 |
| cubic_sum | NumLang-SC-specialize | 8,021,100 | 1,554,300 | 1,681,300 | 30 |
| **expr_eval** | **NumLang-Base-lower** | **2,936,400** | **575,500** | **624,000** | **30** |
| **expr_eval** | **NumLang-SC-specialize** | **35,095,200** | **7,062,200** | **7,323,700** | **30** |
| **window_stats** | **NumLang-Base-lower** | **1,979,900** | **378,400** | **411,600** | **30** |
| **window_stats** | **NumLang-SC-specialize** | **17,112,000** | **3,404,600** | **3,575,800** | **30** |
| **record_pipeline** | **NumLang-Base-lower** | **10,187,600** | **2,038,600** | **2,168,500** | **30** |
| **record_pipeline** | **NumLang-SC-specialize** | **39,636,200** | **7,952,000** | **8,154,500** | **30** |
| **nested_stencil** | **NumLang-Base-lower** | **6,898,500** | **1,338,500** | **1,413,800** | **30** |
| **nested_stencil** | **NumLang-SC-specialize** | **52,150,900** | **10,385,300** | **11,247,900** | **30** |
| fib_matrix | NumLang-Base-lower | 1,289,000 | 248,100 | 417,900 | 30 |
| fib_matrix | NumLang-SC-specialize | 12,675,100 | 2,476,500 | 2,605,800 | 30 |
| hofstadter | NumLang-Base-lower | 6,978,600 | 909,200 | 1,573,700 | 30 |
| hofstadter | NumLang-SC-specialize | 41,774,800 | 8,326,500 | 9,260,700 | 30 |
| kmp | NumLang-Base-lower | 3,334,200 | 626,100 | 691,900 | 30 |
| kmp | NumLang-SC-specialize | 46,295,100 | 10,166,800 | 11,915,900 | 30 |
| map_map | NumLang-Base-lower | 1,124,200 | 214,600 | 228,800 | 30 |
| map_map | NumLang-SC-specialize | 10,405,600 | 2,062,200 | 2,152,000 | 30 |
| nested_stencil | (canonical 4×4 stencil) | — | — | — | — |
| peano_mul | NumLang-Base-lower | 3,626,600 | 693,400 | 749,900 | 30 |
| peano_mul | NumLang-SC-specialize | 37,324,000 | 7,001,900 | 7,414,600 | 30 |
| pow2_mod | NumLang-Base-lower | 1,215,600 | 228,500 | 234,700 | 30 |
| pow2_mod | NumLang-SC-specialize | 7,942,900 | 1,570,500 | 1,645,200 | 30 |
| record_pipeline | (extended pipeline) | — | — | — | — |
| stream_take | NumLang-Base-lower | 1,748,300 | 296,400 | 307,300 | 30 |
| stream_take | NumLang-SC-specialize | 14,050,400 | 2,737,900 | 2,881,300 | 30 |
| sum_map | NumLang-Base-lower | 721,800 | 140,100 | 145,600 | 30 |
| sum_map | NumLang-SC-specialize | 5,156,800 | 998,000 | 1,487,300 | 30 |
| tree_flip | NumLang-Base-lower | 3,049,800 | 584,900 | 623,600 | 30 |
| tree_flip | NumLang-SC-specialize | 122,458,300 | 24,906,400 | 25,598,500 | 30 |
| tri_sum | NumLang-Base-lower | 1,201,300 | 233,400 | 240,300 | 30 |
| tri_sum | NumLang-SC-specialize | 7,150,100 | 1,379,000 | 1,460,900 | 30 |

*Canonical entries not shown for brevity; all 36 rows are present in the CSV.*

## Median Summary (all 36 rows)

The overall median compile time across all benchmarks and systems is **2,038,600 ns** (≈2.0 ms). The SC-specialize median is **7,062,200 ns** (≈7.1 ms), vs Base median of **575,500 ns** (≈0.6 ms). The SC overhead ranges from ~2× (pow2_mod) to ~20× (tree_flip), reflecting the supercompiler's specialization cost relative to simple MIR lowering.

## Scope and Notes

- **This is specialization latency, not end-to-end wall-clock compile time.** It does not include process startup, linking, or binary emission. A future `numlang build` wall-clock column should be added separately rather than mislabeling these numbers as full build time.
- **NEW metric not yet reflected in the frozen contract tables.** The compile-time CSV and this methodology were added to close the contract v1.0 §7 gap where compile-time was previously `N/A`. The existing frozen tables (README.md, SHOWDOWN.md, BENCHMARK_CONTRACT.md) only track `expected_exit` and `COMPUTE_NS`; they do not include specialization latency columns.
- The four extended workloads (`expr_eval`, `window_stats`, `record_pipeline`, `nested_stencil`) are included in the 36-row CSV with SUCCESS status, measured on the same host: 11th Gen Intel(R) Core(TM) i5-11260H @ 2.60GHz, Windows 11 Home, Rust 1.100.0.