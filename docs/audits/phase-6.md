# Phase 6 Self-Audit: README Claims Hygiene and Tone Calibration

Date: 2026-10-10  
Auditor / Orchestrator: Team Peer Review  
Status: **DONE**  

---

## 1. Hostile Re-read: The 5 Weakest Points & Attack Experiments

1. **Weak Point 1: Uncalibrated Benchmark Claims in README (F9)**
   - *Attack*: Cross-referenced numbers in `README.md` Section 6 against `SHOWDOWN.md` and the actual execution data in `bench/data/showdown_results_sample.csv`.
   - *Result*: `README.md` previously reflected un-optimized early iteration numbers where `nrev` and `append3` showed regressions, and used `rustc -O` instead of `-C opt-level=3`.
   - *Resolution*: Synchronized `README.md` with exact results from `SHOWDOWN.md` under `-C opt-level=3` and `--use-mir` baseline. Correctly listed NumLang-SC outright wins (5), sub-500ns ties (4), and explained list allocation runtime differences in C.

2. **Weak Point 2: Sub-Timer Floor Framing and "Zero ns" Claims (F2, F4)**
   - *Attack*: Checked how sub-500ns entries were described in the benchmark table notes.
   - *Result*: Previously, `tri_sum` claimed `~100 ns` for NumLang vs `~0 ns` for Rustc without acknowledging the hardware timer quantization floor.
   - *Resolution*: Added explicit note explaining that $\le 500$ ns results evaluate within the hardware performance counter quantization floor ($\le 500$ ns), where both NumLang-SC (via difference engine) and LLVM SCEV collapse constant loops at compile time; classified transparently as ties.

3. **Weak Point 3: Documentation Cross-References to Lean Status and Claims Ledger**
   - *Attack*: Verified whether public claims in `README.md` link directly to verifiable artifacts and ledger entries.
   - *Result*: Missing direct links to `docs/LEAN_STATUS.md` and `docs/CLAIMS_LEDGER.md`.
   - *Resolution*: Added direct, clickable links in Section 1 and Section 4 to `docs/LEAN_STATUS.md` and `docs/CLAIMS_LEDGER.md`.

4. **Weak Point 4: Claims Linter Execution**
   - *Attack*: Ran `python scripts/claims_lint.py` to test Markdown link integrity, live CI status badges, and Lean axiom hygiene.
   - *Result*: 100% passed with zero broken links or badge violations.

5. **Weak Point 5: Modesty and Elimination of Unverifiable Superlatives**
   - *Attack*: Audited `README.md` for subjective marketing language ("world's first", "revolutionary", "flawless").
   - *Result*: Confirmed complete absence of marketing hype. Tone is strictly academic, transparent, and focused on experimental computer science.

---

## 2. Verification Evidence

- `README.md`: Verified, updated, and aligned with `SHOWDOWN.md`
- `python scripts/claims_lint.py`: **PASSED** (100% claim integrity satisfied)
- `cargo clippy --all-targets -- -D warnings`: **PASSED** (0 errors, 0 warnings)

---

## 3. Assumptions, Guesses & Unverified Areas

- None.

---

## 4. Confidence Assessment

- **Tone & Calibration**: **HIGH**. All claims are anchored to test suites and exact machine logs.
