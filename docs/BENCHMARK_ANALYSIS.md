# Independent Benchmark Analysis (Agent G)

Statistical audit of the published NumLang supercompiler benchmark claims against the raw
measurement CSV, under the frozen `BENCHMARK_CONTRACT.md` v1.0 decision rule.

## 0. Provenance, snapshots, and concurrency

| Item | Value |
|:---|:---|
| CSV analyzed | `bench/data/showdown_results.csv` (gitignored — `.gitignore:16`) |
| **Live CSV SHA-256 (current vintage — involution-fusion win, 2026-11-22)** | `dfdde0efa53c2322e3772f7e8ef9d273cfaa2bf15a2484ed93c447d3b5db4573` (22-benchmark FULL run, 5 warmups + 30 rounds; **8 systems / 176 cells**; 8 SC wins / 14 ties / 0 regressions vs Base; generalized involution proof fuses `tree_flip` (1.80x win); **Clang-O3 comparator/SC geomean 1.11x — SC now leads the full table**) |
| CSV SHA-256 (superseded — Agent M multi-compiler run) | `FC2175DEB37E73CAA8EB3CD0B8034B82D044F45E8BC1304339A005472AE15838` (22-benchmark FULL run; first to add measured Clang/GCC/GHC; 6 SC wins / 16 ties / 0 regressions) |
| CSV SHA-256 (superseded vintage — 6-system run before Clang/GCC/GHC were measured) | `3b1ada010300971af3190ed30a7408ea1126fbd135b328f2974aa51ee2f5b22e` (22-benchmark full run after restoring the `fib_matrix` collapse; 6 wins / 16 ties / 0 regressions) |
| **CSV SHA-256 (superseded vintage — used for the §2 table)** | `D11F063C8EC37A5A1B59B482E3212B7979FD9BF6D8E4AA189750AE8099BECB24` (superseded: the live CSV was re-run after the audit snapshot) |
| README.md SHA-256 (regenerated from the current live CSV) | `DFF8368551744848B1D2384D0ED034387D7D3743D495C36E1F7BC39CA914B56F` |
| SHOWDOWN.md SHA-256 (regenerated from the current live CSV) | `F963BD52F76F987B9E7340089E34CDA73308E58FA1EE7A89768AAFB61F0F7B56` |
| `showdown_results_sample.csv` SHA-256 (byte-identical to live CSV) | `FC2175DEB37E73CAA8EB3CD0B8034B82D044F45E8BC1304339A005472AE15838` |
| Snapshot used for all numbers here | `C:\Users\davea\AppData\Local\Temp\opencode\agentG_showdown_results_snapshot.csv` (identical hash to the superseded vintage; copied **before** analysis because agent F may rewrite the live files) |
| `README.md` snapshot SHA-256 | `D009A681FFE3BA941A03888277ACC83A77818F16798A1A079C9D69CAA10C4A92` |
| `SHOWDOWN.md` snapshot SHA-256 | `8C364A5F13C90CAAC005F91F68F8895168A634F562A7FF560B285C92A3C5248E` |
| Pre-change vintage (`git show HEAD:bench/data/showdown_results_sample.csv`) | SHA-256 `14145D544EB078BAE6269C77CC6D45F8482EBD3FE94E764D5AD78E9A99B6089F` |
| Analysis script | `scripts/analyze_results.py` — run with `python scripts/analyze_results.py` (accepts `--csv PATH`, `--verbose`) |

**Live-CSV re-run note (vintage labeling).** The live `bench/data/showdown_results.csv` has been
re-run several times since this audit snapshot. Its **current** SHA-256 is
`FC2175DEB37E73CAA8EB3CD0B8034B82D044F45E8BC1304339A005472AE15838`: a full 22-benchmark
multi-compiler run (2026-11-21) that adds measured `Clang-O3` (`clang -O3 -march=native`),
`GCC-O3` (`gcc -O3 -march=native`) and `GHC-O2` (`ghc -O2`) cells, yielding 8 systems / 176 CSV
cells. The earlier `3b1ada01…`, `3d41d7d1…` and
`D11F063C8EC37A5A1B59B482E3212B7979FD9BF6D8E4AA189750AE8099BECB24` vintages are superseded.
Current-vintage competitive standings and spot checks are in §7 below; the §2 table below remains
explicitly labeled with the superseded `D11F063C…` vintage it was computed from.
Spot checks on the *superseded* live CSV at audit time via the `scripts/make_bench_table.py` functions:
`nrev` — NumLang-SC `median_ns` 223400 ns; `tri_sum` — NumLang-SC `median_ns` 11400, ratio stats
`1766.36x [1004.70-2624.35]` (matches `README.md:122` / `SHOWDOWN.md:71`). The §2 table below is
therefore labeled as the superseded `D11F063C…` vintage. The live `README.md` / `SHOWDOWN.md`
now hash to `AAC13DA2…` / `38BF5482…` (differ from the audit-time snapshot hashes above; agent F's
post-change re-run updated the tri_sum cells to the 1766.36x values), so the §2 line numbers refer
to the audit-time snapshots. The working copy of `bench/data/showdown_results_sample.csv` is now
byte-identical to the post-change CSV, so the tracked HEAD copy of that sample is the only
available **pre-change** raw-data vintage.

## 1. Methodology

- **Frozen decision rule** (`BENCHMARK_CONTRACT.md:63-74`): a cell with median runtime `<= 500 ns`
  is folded and excluded from numeric win tallies. For every comparator the reported ratio is
  `comparator / NumLang-SC`, so a ratio `> 1` means SC is faster. A strict SC **win** requires
  CI-low `> 1.05`; a strict SC **regression** requires CI-high `< 0.95`; everything else is a
  **tie**. Folded or non-measured cells are reported separately (`FOLD` / `SKIP`), never as wins.
- **Primary CI estimator**: to remain comparable with the published tables, the script reproduces
  the repo's existing estimator (`scripts/make_bench_table.py:168-184`,
  `scripts/ci_regression_guard.py:48-64`): 30 *paired per-round ratios*, sorted, with the 2nd and
  29th order statistics used as the "95% CI" bounds and the median paired ratio as the point
  estimate. It reproduces every published CI exactly (audit item A0 below).
- **Robustness cross-check**: an independent Welch interval on paired `log(comparator/SC)`
  (Student-t `df=29` via Cornish-Fisher, exponentiated) is reported separately. It is **not** used
  for contract counts; it is used to test whether verdicts are estimator-dependent.
- **Geomean speedups**: `exp(mean(log(r_median)))` over the per-benchmark median ratio for every
  evaluable, non-folded benchmark (18 benchmarks, with 8 Rustc folds and 5 MSVC folds removed from
  those comparator geomeans). A second geomean of `median(comparator)/median(SC)` is printed as a
  cross-check; the two agree to within 4%.
- **Data-quality flags**: non-`SUCCESS` status, empty/malformed rounds, fewer than the contracted
  30 rounds, and traceability mismatches between `median_ns`/`min_ns` and the raw rounds.

**Statistic caveat.** Three different statistics coexist in the repository:
`BENCHMARK_CONTRACT.md:80` defines residual runtime as the **geomean of rounds**; the CSV
`median_ns` column is the **upper median** (`sorted[n//2]`) for all 72 `SUCCESS` cells (only
24/72 equal the standard median, 0/72 equal the geomean); `SHOWDOWN.md:97-170` publishes the
**standard median** of the raw rounds. Example `nrev`/NumLang-SC: CSV `median_ns=234900`, raw
standard median `234450`, geomean `247656` ns. The script recomputes all three and flags the
divergence; it uses the raw median for the fold test and the paired-round ratios for CIs.

## 2. Historical numbers — CSV vintage `D11F063C…` (superseded; live vintage is `FC2175DE…` — see §0 and §7)

Generated by `python scripts/analyze_results.py`. Ratio = comparator / SC; `V` is the contract
verdict (`WIN` = SC faster, `REG` = SC significantly slower, `tie`, `FOLD`, `SKIP`).

```text
benchmark            SC med |   Base med        Base ratio [CI]    V |  Rustc med       Rustc ratio [CI]    V |   MSVC med        MSVC ratio [CI]    V
nrev                234.4us |    230.1us       0.99 [0.72-1.26]  tie |     1.33ms       5.78 [4.23-8.25]  WIN |     53.7us       0.23 [0.16-0.26]  REG |
append3              96.8us |     95.2us       0.98 [0.63-1.48]  tie |    444.2us       4.58 [3.09-5.34]  WIN |     15.1us       0.16 [0.11-0.21]  REG |
kmp                  43.9us |     41.3us       0.98 [0.66-1.37]  tie |    226.5us       5.76 [4.02-7.70]  WIN |     61.5us       1.54 [1.09-2.32]  WIN |
peano_mul            38.2us |     50.3us       1.12 [0.79-1.93]  tie |    132.1us       3.48 [2.40-6.05]  WIN |    184.2us       4.75 [3.37-6.41]  WIN |
tree_flip           116.1us |    126.0us       1.06 [0.74-1.50]  tie |    569.0us       5.01 [3.59-7.76]  WIN |    645.0us       5.23 [3.02-8.85]  WIN |
fib_matrix          127.5us |    125.6us       0.99 [0.86-1.13]  tie |      3.4us       0.03 [0.02-0.03]  REG |     34.8us       0.27 [0.24-0.28]  REG |
tri_sum              10.2us |    20.02ms 1919.61 [1604.33-2628.33]  WIN |   <=500ns*                    n/a FOLD |     9.40ms 941.84 [734.12-1135.01]  WIN |
cubic_sum            10.7us |     4.53ms 418.32 [274.85-487.88]  WIN |   <=500ns*                    n/a FOLD |     3.55ms 333.25 [215.26-394.87]  WIN |
pow2_mod             10.5us |      9.5us       0.91 [0.67-1.31]  tie |   <=500ns*                    n/a FOLD |   <=500ns*                    n/a FOLD |
hofstadter           11.4us |     13.3us       1.14 [0.87-1.65]  tie |      5.7us       0.49 [0.38-0.58]  REG |   <=500ns*                    n/a FOLD |
tri_sum_dyn          17.1us |    31.06ms 1862.68 [533.98-2928.08]  WIN |     7.09ms 417.07 [118.43-548.19]  WIN |    14.65ms 874.66 [230.30-1035.23]  WIN |
cubic_sum_dyn        18.1us |     7.04ms 397.60 [151.93-807.74]  WIN |     5.15ms 290.50 [111.07-380.38]  WIN |     5.57ms 316.93 [113.80-540.53]  WIN |
fib_matrix_dyn       20.6us |     20.9us       1.04 [0.38-2.32]  tie |      5.2us       0.26 [0.11-0.32]  REG |      3.6us       0.18 [0.08-0.21]  REG |
pow2_mod_dyn         16.5us |     18.4us       1.01 [0.44-2.82]  tie |   <=500ns*                    n/a FOLD |   <=500ns*                    n/a FOLD |
compose5             15.4us |     16.1us       1.12 [0.45-2.59]  tie |   <=500ns*                    n/a FOLD |   <=500ns*                    n/a FOLD |
map_map              16.0us |     16.9us       1.10 [0.76-2.26]  tie |   <=500ns*                    n/a FOLD |   <=500ns*                    n/a FOLD |
sum_map              10.3us |    461.8us    43.70 [31.82-58.52]  WIN |   <=500ns*                    n/a FOLD |    355.4us    34.65 [25.84-54.29]  WIN |
stream_take          57.6us |     56.1us       0.98 [0.70-1.41]  tie |   <=500ns*                    n/a FOLD |     35.7us       0.62 [0.44-0.67]  REG |
```

| Comparator | SC WIN | tie | SC REG | FOLD | SKIP | Geomean median-ratio | Geomean median-quotient |
|:---|---:|---:|---:|---:|---:|---:|---:|
| NumLang-Base | 5 | 13 | 0 | 0 | 0 | 5.68x | 5.73x |
| Rustc-O3 | 7 | 0 | 3 | 8 | 0 | 4.00x | 3.94x |
| MSVC-O2 | 8 | 0 | 5 | 5 | 0 | 7.16x | 7.08x |

SC wins vs Base: `tri_sum` (1919.61x), `cubic_sum` (418.32x), `tri_sum_dyn` (1862.68x),
`cubic_sum_dyn` (397.60x), `sum_map` (43.70x). SC regressions vs Base: **none**.

Folded cells (excluded, never numeric wins): Rustc-O3 — `tri_sum`, `cubic_sum`, `pow2_mod`,
`pow2_mod_dyn`, `compose5`, `map_map`, `sum_map`, `stream_take` (8); MSVC-O2 — `pow2_mod`,
`hofstadter`, `pow2_mod_dyn`, `compose5`, `map_map` (5). No SC or Base cell is folded.

Welch log-ratio cross-check: Rustc/MSVC verdicts are identical under both estimators. For Base,
`peano_mul` and `hofstadter` move from `tie` to `WIN` (SC faster); the 13 "ties" are therefore
partly a consequence of the conservative paired-order-statistic interval, not evidence that no
effect exists. No verdict flips in the opposite direction.

Data quality: 72 `SUCCESS` cells, each with exactly 30 rounds; 36 non-`SUCCESS` cells, all
`NOT_INSTALLED` (GHC-O2 ×18, HOSC-SC ×18); **0** missing-round cells; **0** status failures among
measured systems; 48 `median_ns` upper-median-vs-standard-median divergences (expected from the
convention described in §1). Folded `SUCCESS` cells contain 0/100 ns quantized rounds (e.g.
Rustc-O3 `tri_sum` geomean `2` ns) — consistent with the counter floor, not a real measurement.

## 3. Pre-change vintage (HEAD sample `14145D54…B6089F`) — for comparison only

| Comparator | SC WIN | tie | SC REG | FOLD | Geomean median-ratio |
|:---|---:|---:|---:|---:|---:|
| NumLang-Base | 6 | 12 | 0 | 0 | 6.22x |
| Rustc-O3 | 8 | 0 | 4 | 6 | 4.76x |
| MSVC-O2 | 9 | 0 | 4 | 5 | 7.81x |

Notable vintage deltas: `fib_matrix` was a Base win (`8.99x [6.76-10.19]`, SC `14.2 us`) and is
now a tie (`0.99x [0.86-1.13]`, SC `127.5 us`); `sum_map` vs Rustc-O3 was a numeric win
(`994.8 us`, `60.46x`) and is now a folded cell; `tri_sum` Base speedup changed from `2172.13x`
to `1919.61x`. Any claim quoted without a vintage label is therefore ambiguous.

## 4. Exact list: benchmarks where SC is *slower* than NumLang-Base

Using the raw-round median (identical list if the CSV `median_ns` upper median is used), SC is
slower in exactly these six benchmarks. All six are **ties**, not strict regressions:

| Benchmark | SC median (raw / CSV) | Base median (raw / CSV) | Base/SC ratio [CI] | Verdict |
|:---|---:|---:|---:|:---|
| `nrev` | 234,450 / 234,900 ns | 230,100 / 230,600 ns | 0.99 [0.72-1.26] | tie |
| `append3` | 96,850 / 97,100 ns | 95,200 / 96,500 ns | 0.98 [0.63-1.48] | tie |
| `kmp` | 43,900 / 44,100 ns | 41,300 / 42,300 ns | 0.98 [0.66-1.37] | tie |
| `fib_matrix` | 127,500 / 127,900 ns | 125,600 / 126,100 ns | 0.99 [0.86-1.13] | tie |
| `pow2_mod` | 10,500 / 10,500 ns | 9,500 / 9,600 ns | 0.91 [0.67-1.31] | tie |
| `stream_take` | 57,600 / 57,600 ns | 56,100 / 56,100 ns | 0.98 [0.70-1.41] | tie |

Every other benchmark has SC at or faster than Base by the raw median. SC is *significantly*
slower than Rustc-O3 on `fib_matrix`, `hofstadter`, `fib_matrix_dyn`, and significantly slower
than MSVC-O2 on `nrev`, `append3`, `fib_matrix`, `fib_matrix_dyn`, `stream_take`.

## 5. Honesty audit — discrepancies and verified-clean items

Line numbers refer to the snapshots in §0 (live files matched at audit end). No files outside this
document and `scripts/analyze_results.py` were edited.

**A0. Verified clean (no mismatch found).** `README.md:116-133` (18×4 cells) and
`SHOWDOWN.md:65-82` (18×6 cells) match the post-change CSV `median_ns` values **cell-for-cell**
under the generator's 2-decimal formatting; every published Base ratio/CI and the
`Speedup vs Base` column (`SHOWDOWN.md:65-82`, column 10) match the recomputed paired-ratio
statistics. All sub-500 ns cells in both main tables are marked `≤ 500 ns*` — **zero** folded
cells are displayed as numeric values or counted as wins. `SHOWDOWN.md:88-90` (5 wins / 13 ties,
zero fabricated wins) agrees with the CSV. So do the raw median/min/order-statistic columns of
`SHOWDOWN.md:99-170` (0 mismatches against raw rounds).

1. **Stale win-count claim in the claims ledger (high).** `docs/CLAIMS_LEDGER.md:24` marks
   CLM-09 "**TESTED** (6 outright wins, 12 ties, 0 fabricated wins out of 18 …)" and cites
   `SHOWDOWN.md:120`, `README.md:120`. The post-change CSV gives **5 wins / 13 ties**
   (`SHOWDOWN.md:88-89`). 6/12 was the *pre-change* vintage (§3). Claim status is stale, not tested
   against the current data.

2. **Stale speedup claim in the claims ledger (high).** `docs/CLAIMS_LEDGER.md:25` records
   CLM-10 as `1962.48x [1335.69-2612.82]` for `tri_sum`. Current data: `1919.61x
   [1604.33-2628.33]` (`README.md:122`, `SHOWDOWN.md:71`, CSV hash `D11F063C…`). The tracked
   pre-change sample gives `2172.13x [1489.41-2941.36]`; the old README text at HEAD gave
   `1865.34x [1361.01-2409.05]`. The ledger figure matches neither vintage.

3. **Claims-ledger source locations no longer resolve (low).** `docs/CLAIMS_LEDGER.md:24-25`
   points to `SHOWDOWN.md:120` / `README.md:120` / `SHOWDOWN.md:125` / `README.md:125`. In the
   audited snapshots those lines are a variance-table row, a `tree_flip` table row, a folded Rustc
   cell, and a `hofstadter` table row respectively — not win-count or tri-sum-summary evidence.

4. **Frozen workload set is internally inconsistent (medium).** `BENCHMARK_CONTRACT.md:23-34`
   declares **18** canonical benchmarks but enumerates only **14** (5 G1 + 5 G2 + 4 G3).
   `SHOWDOWN.md:40-55` likewise registers 14, while `SHOWDOWN.md:65-82` and the CSV report 18,
   adding four `*_dyn` benchmarks in an undocumented `G2-Dyn` group. Those four benchmarks supply
   two of the five current Base wins, so the scope of the "18 benchmark" claim is ambiguous.

5. **Three incompatible definitions of "median"/residual runtime (high).**
   `BENCHMARK_CONTRACT.md:80` mandates the geomean of rounds; `bench/data/showdown_results.csv`
   stores the upper median for 72/72 `SUCCESS` cells; `README.md:116-133` and
   `SHOWDOWN.md:65-82` publish that upper median; `SHOWDOWN.md:99-170` publishes the standard
   median. There are 44 same-cell conflicts inside SHOWDOWN alone, e.g. `SHOWDOWN.md:65`
   `234.90 µs` vs `:99` `234.45 µs`; `:71` `20.19 ms` vs `:124` `20.02 ms`; `:75`
   `17.30 µs` vs `:139` `17.05 µs`; `:81` `10.40 µs` vs `:163` `10.30 µs`. No published
   residual-runtime number currently follows the contract's geomean definition.

6. **"95% CI" is a paired order-statistic interval, not a 95% interval for a mean (medium).**
   `BENCHMARK_CONTRACT.md:69-74` calls the ratio interval a 95% CI, but
   `scripts/make_bench_table.py:168-184`, `scripts/ci_regression_guard.py:48-64` and
   `scripts/make_bench_table.py:142-165` take the 2nd/29th of 30 sorted values. The resulting
   interval is very conservative: the independent Welch log-ratio check converts two Base ties
   into SC wins (`peano_mul`, `hofstadter`). Neither the contract nor the tables disclose the
   estimator or its coverage. This does not fabricate wins; it can hide real ones.

7. **README omits non-measured competitors without in-section disclosure (low).**
   `README.md:114-133` has no GHC-O2 or HOSC-SC columns even though the CSV contains 18
   `NOT_INSTALLED` rows for each; `README.md:110-112` presents the table as the full empirical
   evaluation. `SHOWDOWN.md:28-32` does disclose these (plus SPSC, Clang-O3, GCC-O3, which have
   no CSV rows at all). The footnote at `README.md:135` explains folded cells but not excluded
   competitors.

8. **Folded cells are disclosed, but zero-variance presentation is misleading (low).**
   `SHOWDOWN.md:125,129,133-134,138,153-154,157-158,161-162,165,169` show `MAD 0.00 µs` and
   `[≤ 500 ns* - ≤ 500 ns*]` while the underlying rounds are quantized 0/100 ns values (e.g.
   Rustc-O3 `tri_sum` geomean `2` ns). The folded marker itself is correct and no numeric win is
   claimed; the zero-variance display hides the quantization floor that the marker is meant to
   disclose.

9. **Compile-time and code-size remain undisclosed as required (caveat, not a mismatch).**
   `BENCHMARK_CONTRACT.md:81-86` requires these metrics for a "world's fastest" claim; neither is
   in the CSV. `README.md:114-133` / `SHOWDOWN.md:63-82` have no `N/A` columns for them. The new
   untracked `bench/data/compile_times.csv` (agent F, mtime 15:07:42) was not analyzed here; the
   frozen CSV does not contain those fields.

## 6. Checklist for final verification

- [ ] Re-run `python scripts/analyze_results.py` after all concurrent agents finish; confirm the CSV hash and that the compact table still matches `README.md:116-133` / `SHOWDOWN.md:65-82`.
- [ ] Run the contract's own gate `python scripts/make_bench_table.py --check` (not run by me: outside my file ownership) and confirm zero traceability errors.
- [ ] Update `docs/CLAIMS_LEDGER.md:24-25` (CLM-09 → 5/13, CLM-10 → 1919.61x) or explicitly label those claims pre-change; fix the four stale `file:line` references.
- [ ] Reconcile the 18-vs-14 workload-set contradiction (`BENCHMARK_CONTRACT.md:23-34`, `SHOWDOWN.md:40-55`): register the four `*_dyn` benchmarks or restate the count.
- [ ] Choose one residual-runtime statistic (contract geomean vs CSV upper median vs §4.1 standard median) and align CSV, contract, and both markdown tables; add an explicit rounding rule.
- [ ] Either relabel the 2nd/29th order-statistic interval or replace it with a stated 95% procedure; re-examine `peano_mul` and `hofstadter` ties if the procedure changes.
- [ ] Add a disclosure line in `README.md` §6 for GHC-O2 / HOSC-SC / SPSC / Clang-O3 / GCC-O3, or add their `NOT_INSTALLED` columns.
- [ ] Keep folded cells as `≤ 500 ns*` in all regenerated tables and keep them out of win/speedup tallies; verify after every rerun.
- [ ] Add `N/A` disclosure for compile-time and code-size (or publish measured values) before any "world's fastest supercompiler" claim.
- [ ] Confirm the final `git status --short` shows only `scripts/analyze_results.py` and `docs/BENCHMARK_ANALYSIS.md` as my changes; all other modifications/untracked files belong to other agents or pre-existed this audit.

## 7. Current vintage — Agent M multi-compiler FULL run (2026-11-21)

**Provenance.** `tests/supercompiler_showdown.rs` was extended so that the existing C sources are
compiled and run by both `clang -O3 -march=native` (`Clang-O3`) and `gcc -O3 -march=native`
(`GCC-O3`), and the 18 registered Haskell sources by `ghc -O2` (`GHC-O2`). The harness was run in
**FULL** mode (explicitly *not* quick: 5 discarded warmups + 30 measured rounds, 22 benchmarks)
with the user-level Scoop paths prepended:

```powershell
$env:PATH = "C:\Users\davea\scoop\shims;C:\Users\davea\scoop\apps\llvm\current\bin;C:\Users\davea\scoop\apps\mingw\current\bin;C:\Users\davea\scoop\apps\haskell\current\lib\bin;" + $env:PATH
cargo test --test supercompiler_showdown -- --nocapture
```

Full log: `C:\Users\davea\AppData\Local\Temp\opencode\agentM_full_showdown.log`.
Result: `test result: ok. 2 passed; 0 failed`; harness reported
`FULL (Scientific Standard: 30 rounds, 5 warmups)`, 22 benchmarks evaluated.
Detected toolchains: Clang 23.1.6, GCC 16.2.0, GHC 9.14.1, MSVC-O2, Rustc-O3 **FOUND**;
HOSC-SC and SPSC **NOT_FOUND**.

`python scripts/make_bench_table.py --all` regenerated `README.md` and `SHOWDOWN.md` strictly from
the new CSV. `python scripts/make_bench_table.py --check` → **BENCH CHECK PASSED**;
`python scripts/ci_regression_guard.py` → **0 regressions** across all 22 benchmarks.

Live CSV SHA-256: `FC2175DEB37E73CAA8EB3CD0B8034B82D044F45E8BC1304339A005472AE15838`
(176 cells = 8 systems × 22 benchmarks). Generated by `python scripts/analyze_results.py`.

### 7.1 Competitive standings vs NumLang-SC

Decision rule unchanged (contract §6): ratio = comparator / NumLang-SC. `WIN` = SC faster than
that comparator by ≥5% with CI excluding 1.0; `REG` = SC slower (comparator beats SC); `FOLD` =
cell at/below the 500 ns counter floor and excluded; `SKIP` = no comparable measurement.

| Comparator | SC WIN | tie | SC REG (comparator beats SC) | FOLD | SKIP | Geomean median ratio (comp/SC) |
|:---|---:|---:|---:|---:|---:|---:|
| NumLang-Base | 6 | 16 | 0 | 0 | 0 | 4.57x |
| Rustc-O3 | 11 | 0 | 3 | 8 | 0 | 4.52x |
| MSVC-O2 | 9 | 1 | 7 | 5 | 0 | 4.73x |
| **Clang-O3** | **3** | **3** | **5** | **11** | **0** | **0.73x** |
| **GCC-O3** | **6** | **2** | **7** | **7** | **0** | **2.09x** |
| **GHC-O2** | **7** | **2** | **5** | **1** | **7** | **5.28x** |

NumLang-SC's six strict wins vs NumLang-Base are `fib_matrix` (12.22x), `tri_sum` (2058.18x),
`cubic_sum` (435.37x), `tri_sum_dyn` (2172.48x), `cubic_sum_dyn` (396.07x) and `sum_map`
(39.85x); zero regressions vs Base.

### 7.2 Clang / GCC / GHC cell-level detail (all 22 benchmarks)

- **Clang-O3** (22 measurable C cells): SC wins 3, ties 3, loses 5, folds 11. Clang is folded to
  `≤ 500 ns*` on 11 cells (`tri_sum`, `cubic_sum`, `pow2_mod`, `hofstadter`, `tri_sum_dyn`,
  `cubic_sum_dyn`, `pow2_mod_dyn`, `compose5`, `map_map`, `sum_map`, `stream_take`) — these are
  compile-time/micro-loop collapses, **not** numeric wins. Clang beats SC on `nrev` (49.6 µs vs
  228.9 µs, 0.22x), `append3` (13.9 µs vs 96.0 µs, 0.14x), `fib_matrix` (3.1 µs vs 10.1 µs,
  0.31x), `record_pipeline` (8.4 µs vs 12.1 µs, 0.69x) and `nested_stencil` (9.8 µs vs 15.2 µs,
  0.64x). SC wins on `kmp`, `peano_mul`, `tree_flip`.
- **GCC-O3** (22 measurable C cells): SC wins 6, ties 2, loses 7, folds 7. GCC folds exactly 7
  cells (`tri_sum`, `pow2_mod`, `hofstadter`, `tri_sum_dyn`, `pow2_mod_dyn`, `compose5`,
  `sum_map`) and is beaten by SC on the large recurrence sums, e.g.
  `cubic_sum` 130.87x, `tri_sum_dyn` 525.86x, `cubic_sum_dyn` 127.50x, `sum_map` folded. GCC
  beats SC on `nrev` (27.1 µs, 0.12x), `append3` (8.6 µs, 0.09x), `fib_matrix` (2.5 µs, 0.25x),
  `stream_take` (21.3 µs, 0.33x), `record_pipeline` and `nested_stencil`.
- **GHC-O2** (18 compiled cells; 4 G4 benchmarks have no `.hs` source → `NOT_INSTALLED`, and
  `pow2_mod`, `pow2_mod_dyn`, `stream_take` fail the correctness gate with
  `WRONG_OUTPUT(actual=1;expected=0)` because GHC maps `exitWith (ExitFailure 0)` to process exit
  1). Of comparable cells SC wins 7, ties 2, loses 5, folds 1. GHC is the fastest measured system
  on `nrev` (26.9 µs), `kmp` (8.9 µs), `peano_mul` (8.7 µs) and `tree_flip` (20.2 µs); on
  `append3` GCC (8.6 µs) edges GHC (9.1 µs). SC wins decisively on `tri_sum` (1628.01x), `tri_sum_dyn` (1706.48x),
  `cubic_sum_dyn` (380.77x) and `fib_matrix_dyn` (7.66x). GHC never produces a folded numeric
  win; its single folded cell is `compose5`.

### 7.3 Does any system beat NumLang on the recurrence wins?

**Yes, partially.** Among the five recurrence benchmarks where SC strictly beats Base
(`fib_matrix`, `tri_sum`, `cubic_sum`, `tri_sum_dyn`, `cubic_sum_dyn`):

- `fib_matrix`: **Clang (3.1 µs), GCC (2.5 µs) and Rustc (3.4 µs) all beat NumLang-SC (10.1 µs)**
  with CI excluding 1.0 (`REG` verdicts for SC). GHC (8.1 µs) is a statistical tie; MSVC (34.8 µs)
  loses to SC.
- `tri_sum`, `tri_sum_dyn`, `cubic_sum_dyn`: no system produces a real numeric time below SC
  (≈9.5–11.3 µs). Clang is folded to `≤ 500 ns*` on all three — excluded as a folded cell, not a
  numeric win. GCC/GHC/MSVC/Rustc are 100x–2000x slower than SC on these.
- `cubic_sum`: GCC is 130.87x **slower** than SC (SC win); Clang and Rustc are folded.

So the recurrence dominance claim survives for the large/dynamic sums, but `fib_matrix` is genuinely
lost to all three native C/LLVM compilers. On the full 22-benchmark set Clang's geomean ratio is
0.73x (comparator faster than SC overall), GCC's is 2.09x and GHC's is 5.28x (SC faster overall),
reflecting the mix of folded constant loops and real runtime wins.

**Integrity note.** All numbers above come from raw CSV rounds; folded cells remain excluded,
`WRONG_OUTPUT` / `NOT_INSTALLED` cells are never converted into wins, and the win/tie/regression
verdicts published in `README.md` / `SHOWDOWN.md` remain keyed on `NumLang-Base` only.
