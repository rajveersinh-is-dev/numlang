# Phase 0 Self-Audit: Baseline & Truth Audit

Date: 2026-10-10  
Auditor / Orchestrator: Team Peer Review  
Status: **DONE**  

---

## 1. Hostile Re-read: The 5 Weakest Points & Attack Experiments

1. **Weak Point 1: Deforestation Slowdown on 5 Benchmarks (`nrev`, `append3`, `kmp`, `peano_mul`, `tree_flip`)**
   - *Attack*: Ran `tests/supercompiler_showdown.rs` directly and compared `NumLang-SC` vs `NumLang-Base`.
   - *Result*: Confirmed that `NumLang-SC` is 0.59x to 0.87x slower than `NumLang-Base`. Residual synthesis creates additional AST call wrappers that perform worse than the unoptimized baseline loop.
   - *Remediation Planned (Phase 3)*: Implement a strict profitability gate so the compiler falls back to baseline if residual cost exceeds baseline cost.

2. **Weak Point 2: Showdown Sub-Timer Resolution & False Wins**
   - *Attack*: Examined measurement rows where timings fell to 0 ns or 100 ns (`tri_sum`, `cubic_sum`, `compose5`, `pow2_mod`).
   - *Result*: Confirmed that measuring instant compile-time closed-form evaluations in microbenchmarks is quantized at the timer resolution (~100 ns). Rustc shows 0 ns, yet NumLang-SC was designated the winner.
   - *Remediation Planned (Phase 4)*: Downgraded CLM-09 in `docs/CLAIMS_LEDGER.md`. Showdown harness must mark sub-timer rows as "Instantaneous / Sub-timer floor" and forbid ranking them as numerical speedup wins.

3. **Weak Point 3: Synthetic Template Inlining in `distill.rs`**
   - *Attack*: Grepped `src/` for benchmark function names and identified `synthesize_append3` in `src/mir/supercompiler/distill.rs:942-964` constructing a hardcoded `GlobalTerm::Call { func: "append".to_string() }`.
   - *Result*: While `integrity_lint.rs` checked for `== "append"`, the compiler synthesized a process tree hardcoding `"append"`.
   - *Remediation Planned (Phase 1/3)*: Replace hardcoded template trees with purely structural, inductive anti-unification and generalized folding.

4. **Weak Point 4: Recurrence Solver Overflow Before Division**
   - *Attack*: Checked `solve_recurrence` in `src/mir/supercompiler/generalize.rs:201-206` where $k(k-1)$ is multiplied before dividing by 2.
   - *Result*: For $k = 4 \times 10^9$ (valid in i64), $k(k-1)$ overflows 64-bit signed wrapping integers before division by 2, yielding erroneous closed-form answers.
   - *Remediation Planned (Phase 2)*: Refactor formula to evaluate parity first: `(k % 2 == 0 ? (k / 2) * (k - 1) : k * ((k - 1) / 2))`.

5. **Weak Point 5: Lean 4 Termination Scope Gap**
   - *Attack*: Audited `proof/NumLangProofs/Termination.lean:276-339` for the claim of Kruskal's Tree Theorem mechanization.
   - *Result*: The Lean formalization mechanizes the finite-alphabet Pigeonhole Principle (`pigeonhole_seq`), not Kruskal's Tree Theorem over unbounded terms.
   - *Remediation Planned (Phase 5)*: Formally state in `docs/LEAN_STATUS.md` that Kruskal's WQO theorem is cited from the literature (Leuschel 1998), while the in-repo Lean code mechanizes finite-alphabet pigeonhole termination and semantic preservation.

---

## 2. Claims Linter Execution

Ran `python scripts/claims_lint.py`:
```
=====================================================================
                 NUMLANG HOSTILE CLAIMS & INTEGRITY LINTER          
=====================================================================
[1/3] Checking local Markdown links and document references...
  [OK] All markdown links point to existing files.
[2/3] Checking live CI status badges in README...
  [OK] No static build badges detected.
[3/3] Checking Lean 4 formalization for 'sorry' and 'axiom' shortcuts...
  [OK] Zero 'sorry' and zero 'axiom' statements found in Lean formalizations.
=====================================================================
PASSED: 100% claim integrity checks satisfied.
```

---

## 3. Assumptions, Guesses & Unverified Areas

- **Unverified**: We have not yet measured whether GHC and HOSC produce genuine stream fusion gains on Linux runners, because host environment is currently Windows. This will be verified in GitHub Actions runner logs during Phase 4.
- **Assumed**: Assumed that the in-tree CDCL bit-blasting solver in `src/mir/supercompiler/validate.rs` covers non-linear arithmetic by uninterpreted functions. This will be formally audited in Phase 2.

---

## 4. Temptations & Shortcuts Avoided

- *Temptation*: To classify F1 as "partially true" by claiming deforestation still eliminates intermediate allocations even if runtime is slower.
  - *Reality*: Acknowledged fully that slower runtime is an optimization regression. Categorized F1 as **CONFIRMED**.
- *Temptation*: To defend "9 of 14 wins" by claiming sub-100ns operations are "effectively zero".
  - *Reality*: Acknowledged that claiming wins on timer-floor noise is dishonest. Downgraded and categorized F2 as **CONFIRMED**.

---

## 5. Confidence Assessment

- **Audit Findings (F1–F12)**: **HIGH**. Every candidate finding verified against exact file and line citations.
- **Claims Ledger**: **HIGH**. Seeded with all 20 public claims across documentation, paper, and roadmap.
- **Integrity Rule Enforcement**: **HIGH**. Clean build, clean tests, `claims_lint.py` passing with 0 errors.

---

## 6. Regression Check

- Baseline Release Build: **PASSED** (`docs/evidence/2026-10-10-build-release.txt`)
- Test Suite Execution: **PASSED** (`docs/evidence/2026-10-10-test-all.txt`, 0 failed)
- Clippy & Formatting: **PASSED** (`cargo clippy --all-targets -- -D warnings`, 0 warnings)
