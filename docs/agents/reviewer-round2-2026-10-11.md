# Sub-Agent Report: Independent Round 2 Evidence Reviewer

**Date:** 2026-10-11  
**Role:** Independent Adversarial Reviewer (Round 2 Evidence Verification)  
**Owned Files:** `docs/agents/reviewer-round2-2026-10-11.md`, cross-phase verification  

---

## 1. Review Scope & Objective
Conduct an independent, adversarial audit of all empirical, statistical, and architectural evidence generated during Round 2 (Phases A through D), verifying strict adherence to Global Rules G1–G5 and eliminating discrepancies between public claims and empirical data.

---

## 2. Independent Verification & Provenance

### 2.1 Empirical & Statistical Checks
- [READ `bench/data/perf_history.csv`]: Verified baseline slowdown diagnosis (R5); confirmed architectural shift to `--use-mir` SSA lowering and in-process `println(res)` flush overhead between commits `7e05c38`, `61a4292`, `fe67d59`, and `627fe49`.
- [READ `docs/evidence/round2/allocator_attribution.csv`]: Verified allocator attribution (R6); confirmed the 25x MSVC vs Rust gap on `nrev` was an allocator artifact (Windows `HeapAlloc` 1,425.5 µs vs bump arena 57.3 µs), and confirmed NumLang-SC and NumLang-Base evaluate at statistical parity under their shared runtime allocator.
- [READ `docs/evidence/round2/ablation_study.csv`]: Verified per-pass ablation measurements across 15 rounds; confirmed stability under cost model.
- [READ `bench/data/showdown_results.csv`]: Verified all 18 literature benchmarks have 30 raw execution rounds recorded.
- [RAN `python scripts/make_bench_table.py --all`]: Confirmed 6 outright speedup wins and 12 statistical ties under strict 95% CI rule ($[r_{\text{low}}, r_{\text{high}}]$ excludes 1.0 AND effect $\ge 5\%$). Zero fabricated wins claimed.
- [RAN `python scripts/ci_regression_guard.py`]: Verified zero benchmarks regressed beyond noise threshold ($\text{upper 95\% CI} \ge 0.95$).

### 2.2 Formal & Invariance Checks
- [READ `src/mir/supercompiler/mrsc.rs:702-775`]: Verified 4-dimensional cost vector evaluation (`dynamic_steps`, `allocations`, `residual_blocks`, `register_pressure`).
- [READ `src/mir/supercompiler/mod.rs`]: Verified profitability gate rejects residuals when $\text{sc\_score} > \text{base\_score}$ and correctly executes `reset_reverted_stats`.
- [RAN `cargo test --test cost_model_invariance_tests`]: Verified 100% pass across all functions in all 18 canonical benchmarks ($\text{score}(\text{SC}) \le \text{score}(\text{Base}) + 10^{-6}$).
- [READ `docs/CLAIMS_LEDGER.md`]: Verified all 20 claims (CLM-01 through CLM-20) updated with real git commit hashes; verified CLM-07 status is explicitly `PROVEN (abstract model only)`.
- [RAN `python scripts/claims_lint.py`]: Verified all 6 stages pass with 0 errors (links, live badges, zero sorry/axioms, Lean docs inventory, claims ledger commit hashes, single source of truth).
- [RAN `cargo clippy --all-targets -- -D warnings`]: Verified 0 errors and 0 warnings.
- [RAN `cargo test`]: Verified entire test suite passes cleanly (0 failures).

---

## 3. Findings Resolution Status
1. **R1–R4 (Recurrence Wins Restored):** Confirmed dynamic recurrence collapses restored (`tri_sum_dyn` 1,874x, `cubic_sum_dyn` 404x, `fib_matrix` 9.07x, `sum_map` 40.02x).
2. **R5–R6 (Baseline Slowdown & Allocator Attribution):** Confirmed transparently documented in `SHOWDOWN.md`, `README.md`, and `docs/audits/round2-phase-B.md`.
3. **R7 (Profitability Gate & Invariance):** Confirmed cost model gate halts regression and invariant test passes 100%.
4. **R8 (Single Source of Truth):** Confirmed `CLAIMS_LEDGER.md`, `SHOWDOWN.md`, and `README.md` numbers are generated strictly from raw CSVs and validated by automated linters.

---

## 4. Final Verdict
**OFFICIALLY SIGNED OFF**: Round 2 empirical, formal, and statistical evidence fully reconciled with public claims under strict computational honesty.
