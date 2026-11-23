# Weak-point plan — standardized 22-benchmark suite

Baseline: commit `99dbcf4`, CSV SHA-256 `9388e20826435c0466bf923b3171a47a25c460f9639e9d072da60ab8944bc76d`.
Contract: ratio = comparator / NumLang-SC; WIN iff CI low > 1.05, REG iff CI high < 0.95,
cells at <= 500 ns are folded. Full harness is the only source of truth.

## 1. Inventory of weak points (from `scripts/analyze_results.py`)

### Tier 1 — outright REGRESSIONS (2 kernels, 5 cells)
| kernel | SC | competitors that beat us | note |
|---|---|---|---|
| `fib_matrix_dyn` | 5.5 us | rustc 0.83x, MSVC 0.60x, Clang 0.56x, GCC 0.56x | outer `sum = (sum + C) % M` loop not collapsed; C = fib(40) = 102334155, M = 1e9+7, N from `read_i64` |
| `kmp` | 27.7 us | GHC 0.26x (7.5 us) | list-based DFA over a freshly built 15-cell list, repeated 100x |

### Tier 2 — TIES vs Base that Welch 95% CI already calls WIN (5 kernels)
`compose5` (1.21), `record_pipeline` (1.76), `expr_eval` (1.21), `window_stats` (1.10),
and `nested_stencil` (0.90 vs Base, GCC tie 1.32). These sit at the ~1.6–6.8 us
fixed-overhead floor: shrinking the per-run constant turns them into robust wins.

### Tier 3 — other TIEs vs Base (small margin, noise-sensitive)
`pow2_mod` (1.07), `pow2_mod_dyn` (1.19), `map_map` (1.06), `hofstadter` (1.09,
also tie vs rustc 1.30), `fib_matrix_dyn` (1.01), `kmp` (0.98).

### Tier 4 — narrow WINs to harden (CI low barely above 1.05)
`fib_matrix` vs GCC 1.56 [1.09-1.79]; `nested_stencil` vs Clang 1.58 [1.25-2.31]
and MSVC 1.66 [1.22-2.90]; `window_stats` vs MSVC 2.14 [1.15-4.09];
`expr_eval` vs MSVC tie 2.57 [0.92-4.07].

## 2. Wave plan (each wave re-targets whatever remains weakest)

Wave 1 — five parallel hy4 agents in isolated worktrees:
- **A: fib_matrix_dyn** — sound O(1) modular-affine recurrence collapse, driven by
  SSA/algebraic value identity (never by alias names). Must handle runtime N.
- **B: kmp** — eliminate repeated `make_text(15)` allocation and fuse the DFA
  dispatch; target < 10 us.
- **C: nested_stencil** — unroll k<9 neighbourhood, hoist dr/dc/coeff and index
  arithmetic, fuse the copy-back pass; target < 4 us.
- **D: overhead floor** — shrink the ~1.6 us SC constant (entry stub, read_i64,
  print helper, Cranelift prologue) to flip Tier-2/3 ties into wins.
- **E: hofstadter** — collapse the coupled f/m recurrence below the 3.4 us mark
  and harden narrow wins.

Integration rule: each agent commits in its own worktree after correctness tests
and targeted A/B timing. The orchestrator cherry-picks only patches that pass the
FULL standardized harness with zero regressions and improved geomeans; anything
else is discarded. Repeat waves on the newly weakest kernels until no measured
improvement remains.

## 3. Verification protocol for every candidate
1. `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings`
2. `cargo test` (full suite, especially differential/closed-form/coupling tests)
3. 9+ interleaved A/B runs of the target kernel via
   `cargo run --release --bin numlang -- run --supercompile --mode distill --no-cache --bench <file>`
4. Full `cargo test --test supercompiler_showdown -- --nocapture` integrated run
5. `python scripts/make_bench_table.py --all`, `scripts/analyze_results.py`,
   `scripts/claims_lint.py`, `scripts/make_bench_table.py --check`,
   `scripts/ci_regression_guard.py`
6. Publish only if geomeans improve and regressions stay at zero.
