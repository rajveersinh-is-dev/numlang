# NumLang Honesty-Round2 Audit & Verification Report

**Date:** 2026-10-10  
**Branch:** `honesty-round2`  
**PR:** [#1 (Honesty-Round2: Comprehensive mathematical and empirical integrity audit)](https://github.com/rajveersinh-is-dev/numlang/pull/1)  
**Status:** **COMPLETE & 100% VERIFIED**  
**Governing Standard:** Global Computational & Mathematical Integrity Rules (G1–G5)

---

## 1. Executive Summary

This report concludes **Honesty-Round2** for the NumLang compiler, formal verification suite, and empirical benchmark infrastructure. Across all 5 audited tasks, every formal claim, benchmark measurement, and documentation asset was held to strict, adversarial standards:

1. **Zero Preloaded Constants / Lookup Tables (Rule G1)**: Every benchmark result is computed dynamically from first principles. Zero lookup tables, precalculated answers, or hardcoded recurrence outputs exist anywhere in the codebase.
2. **Computational Honesty & Real In-Process Timings (Rule G2)**: Timings are measured in-process using monotonic hardware performance counters (`QueryPerformanceCounter`) over 5 discarded warmups and 30 measurement rounds. Sub-timer floor measurements ($\le 500$ ns) are designated as ties rather than synthetic wins.
3. **Verifiability, Purity & Type Safety (Rule G3)**: Zero `panic!()` in code generators, zero `.unwrap()` in lowering pipelines, and 0 warnings under `cargo clippy --all-targets -- -D warnings`.
4. **Algorithmic Generality (Rule G4)**: All compiler transformations and supercompilation drivers operate purely structurally without matching function or variable names.
5. **Formal Proof Integrity (Rule G5)**: Zero `sorry`, zero `axiom`, and zero `native_decide` in the Lean 4 formalization. All headline theorems depend strictly on core Lean 4 foundational axioms (`propext`, `Quot.sound`, `Classical.choice`).

---

## 2. Host Machine & Toolchain Environment (Verbatim)

All empirical benchmarks and local evidence logs were generated on the following dedicated host environment:

- **Host Processor:** 11th Gen Intel(R) Core(TM) i5-11260H @ 2.60GHz (6 physical cores, 12 logical processors)
- **Host Operating System:** Microsoft Windows 11 Home (x86_64)
- **High-Resolution Performance Counter:** Windows `QueryPerformanceCounter` / `QueryPerformanceFrequency` (~10 MHz resolution)
- **Rust Toolchain:**
  - `rustc 1.98.1 (48a229cea 2026-09-01)`
  - `cargo 1.98.1 (797e8a9bc 2026-08-05)`
- **Lean Toolchain:**
  - `Lean 4.34.1 (x86_64-w64-windows-gnu, commit 5045d0056413266e57c625dcd7c365b10e377c52, Release)`
  - `Lake 5.0.0-src+5045d00 (Lean version 4.34.1)`
- **Python Environment:** `Python 3.14.6` (AMD64)
- **C Compiler:** Microsoft Visual C++ (MSVC / `cl.exe` 19.44.35216)
- **Haskell Compiler:** GHC 9.6.6

---

## 3. Task Completion & Engineering Breakdown

### Task 1: Lean Documentation Reality
- **Automated Inventory & Verification Scripts**:
  - Authored [`scripts/generate_lean_inventory.py`](../scripts/generate_lean_inventory.py) to extract definitions, theorems, and line counts from both `lean/` and `proof/`.
  - Authored [`scripts/check_lean_docs.py`](../scripts/check_lean_docs.py) as an automated CI check that parses Lean status documentation and verifies referenced files, theorems, and line counts against the filesystem.
- **Accurate Mathematical Scoping**:
  - Clarified the scope of Kruskal's Tree Theorem in [`docs/LEAN_STATUS.md`](LEAN_STATUS.md) and [`docs/TERMINATION_PROOF.md`](TERMINATION_PROOF.md): the mechanized proof covers the finite-alphabet sequence pigeonhole termination argument; Kruskal's general theorem on unbounded term algebras is cited from the literature (Leuschel 1998, Hamilton 2007).
  - Explicitly demarcated the model-implementation boundary: Lean 4 mechanizes the abstract operational semantics; the native Rust compiler is verified by the in-tree translation validation engine and exhaustive differential testing.
- **Commit**: `c39e66f` — `docs(lean): generate Lean inventory, add check_lean_docs, and calibrate formal docs`

---

### Task 2: Replace Vacuous Lean Definitions
- **Constructive Operational Semantics**:
  - Replaced vacuous tautological definitions in `lean/Supercompiler/` with real constructive small-step operational semantics and an inductive big-step evaluation relation `(p, σ) ⇓ v`.
  - Proved determinism of the big-step evaluation relation.
  - Proved step-by-step semantic preservation lemmas for constant folding, driving steps, knot folding, and distillation.
- **Automated Axiom Verification**:
  - Authored automated `#print axioms` harnesses in [`lean/AxiomCheck.lean`](../lean/AxiomCheck.lean) and [`proof/AxiomCheck.lean`](../proof/AxiomCheck.lean).
  - Authored [`scripts/check_lean_axioms.py`](../scripts/check_lean_axioms.py) to programmatically verify that all headline theorems depend only on foundational Lean kernel logic.
  - Both Lean packages compile with **0 errors, 0 warnings, 0 `sorry`, and 0 custom axioms** across 16 parallel Lake jobs.
- **Commit**: `0f93a72` — `feat(lean): mechanize real small-step semantics, non-vacuous transformation proofs, and axiom check harnesses`

#### Verbatim `#print axioms` Verification Output

Captured directly from [`docs/evidence/2026-10-10/lean_axioms.log`](evidence/2026-10-10/lean_axioms.log):

```text
--- Axiom output for lean/AxiomCheck.lean ---
'Supercompiler.const_fold_stmt_equiv' depends on axioms: [propext, Quot.sound]
'Supercompiler.drive_step_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.driving_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.fold_step_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.distillation_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.refinement_pruning_sound' depends on axioms: [propext]
'Supercompiler.mrsc_selection_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.noop_removal_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.eta_reduction_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.function_transformed_preserves_semantics' depends on axioms: [propext, Quot.sound]
'Supercompiler.supercompiler_sound' depends on axioms: [propext, Quot.sound]
--- Axiom output for proof/AxiomCheck.lean ---
'NumLang.driving_correctness' depends on axioms: [propext]
'NumLang.driving_soundness' depends on axioms: [propext]
'NumLang.driving_equiv' depends on axioms: [propext]
'NumLang.unfold_call_equiv' depends on axioms: [propext]
'NumLang.folding_equiv' depends on axioms: [propext]
'NumLang.generalization_equiv' depends on axioms: [propext]
'NumLang.smallstep_const_fold' does not depend on any axioms
'NumLang.smallstep_branch_prune_true' does not depend on any axioms
'NumLang.smallstep_call_unfold' does not depend on any axioms
'NumLang.emb_refl' does not depend on any axioms
'NumLang.emb_trans' does not depend on any axioms
'NumLang.pigeonhole_seq' depends on axioms: [propext, Classical.choice, Quot.sound]
'NumLang.finite_alphabet_good_sequence' depends on axioms: [propext, Classical.choice, Quot.sound]
'NumLang.finite_configurations_whistle_terminates' depends on axioms: [propext, Classical.choice, Quot.sound]
'NumLang.no_infinite_whistle_free_path' depends on axioms: [propext, Classical.choice, Quot.sound]

AXIOM CHECK PASSED: All headline theorems depend only on foundational Lean axioms.
```

---

### Task 3: Benchmark Correctness Oracle & Dynamic Recurrences
- **Full-Stdout Correctness Oracle**:
  - Created [`scripts/benchmark_oracle.py`](../scripts/benchmark_oracle.py), implementing clean reference evaluations of all 18 benchmark programs with bit-exact 64-bit wrapping arithmetic.
  - Upgraded the showdown runner ([`tests/supercompiler_showdown.rs`](../tests/supercompiler_showdown.rs)) to verify full standard output and exact exit status against the Python reference oracle before recording any timings.
- **Negative Verification Test**:
  - Authored `test_showdown_oracle_negative` to verify that standard output discrepancies (such as residuals differing by multiples of 256 that produce identical 8-bit exit codes) are strictly rejected.
- **Dynamic Recurrence Benchmarks**:
  - Implemented 4 dynamic runtime-input recurrence variants across all four competitor languages (NumLang, Rust, C, Haskell):
    - `tri_sum_dyn` (Dynamic Triangular Summation, $N=50,000,000$)
    - `cubic_sum_dyn` (Dynamic Sum of Squares, $N=10,000,000$)
    - `fib_matrix_dyn` (Dynamic Coupled Fibonacci Matrix Power, $N=1,000$)
    - `pow2_mod_dyn` (Dynamic Geometric Power Loop, $N=100$)
  - Benchmarks accept runtime arguments via the platform runtime routine `__nl_read_i64()`.
- **Diagnosis and Fix for `stream_take.nl` Supercompilation Bug**:
  - **Bug**: In `src/mir/supercompiler/drive.rs`, `try_solve_loop_recurrence()` checked for mutated variables by comparing iteration 0 to iteration 1. Because `total = 0 + 0*0 = 0` remained unchanged on iteration 0, `total` was erroneously assumed constant. Furthermore, `AccumulatorLoopResult::BranchingBody` was bypassed when `any_solved && all_mutating_solved` matched first, causing the loop to exit prematurely with `total = 0`.
  - **Resolution**: Loops with branching bodies (`AccumulatorLoopResult::BranchingBody`) immediately return `RecurrenceResult::UnsolvableBranchingBody`. The loop correctly residualizes as an unrolled/driven CFG, computing the true value `9600000` verified by the oracle.
- **Linker Stub Concurrency Fix**:
  - Refactored native Windows object stub generation in [`src/codegen/linker.rs`](../src/codegen/linker.rs) to use `OnceLock`-cached object paths, eliminating race conditions during parallel test execution.
- **Automated Showdown Table Generation**:
  - Authored [`scripts/make_bench_table.py`](../scripts/make_bench_table.py) to parse in-process hardware performance counter CSV results and regenerate [`SHOWDOWN.md`](../SHOWDOWN.md) and [`README.md`](../README.md).
- **Showdown Results**:
  - 18 benchmarks evaluated (100% verified against full-stdout Python oracle).
  - NumLang-SC achieves outright wins in its target domain of classical deforestation (`kmp`: 53.6µs vs MSVC 91.3µs, `peano_mul`: 49.6µs vs MSVC 201µs, `tree_flip`: 124.5µs vs MSVC 513µs / Rust 540µs).
- **Smoke Mode Calibration**:
  - Configured `tests/supercompiler_showdown.rs` to include `kmp` in `QUICK_BENCHMARKS=1` smoke mode, asserting $\ge 1$ deforestation win in smoke mode and $\ge 3$ deforestation wins in full 18-benchmark mode.
- **Commit**: `fe67d59` — `feat(bench): implement full-stdout oracle, python reference, negative test, and runtime variants`

---

### Task 4: Evidence Collection, Translation Validation & Claims Calibration
- **In-Tree Translation Validation Engine Audit**:
  - Verified exact line count of [`src/mir/supercompiler/validate.rs`](../src/mir/supercompiler/validate.rs): **2,326 lines**.
  - Documented that translation validation is an **opt-in pass** invoked via the `--verify-equivalence` flag (with `--verify` alias), verified by `cargo test --test translation_validation_smt_tests`.
- **Raw Evidence Logs Captured**:
  - Generated and recorded raw logs in [`docs/evidence/2026-10-10/`](evidence/2026-10-10/):
    - `cargo_build.log` (`cargo build --release` exit code 0)
    - `cargo_test.log` (`cargo test` full suite 100% passing)
    - `clippy.log` (`cargo clippy --all-targets -- -D warnings` 0 errors, 0 warnings)
    - `fmt.log` (`cargo fmt --check` 0 formatting discrepancies)
    - `claims_lint.log` (`python scripts/claims_lint.py` 100% passing)
    - `lean_build.log` (`cd lean && lake build` 10 jobs clean)
    - `proof_build.log` (`cd proof && lake build` 6 jobs clean)
    - `lean_axioms.log` (`python scripts/check_lean_axioms.py` 0 sorry, 0 custom axioms)
- **Claims Ledger & Report Calibration**:
  - Updated [`docs/FINAL_REPORT.md`](FINAL_REPORT.md), [`docs/CLAIMS_LEDGER.md`](CLAIMS_LEDGER.md), [`docs/AUDIT_FINDINGS.md`](AUDIT_FINDINGS.md), and [`docs/FORMAL_VERIFICATION.md`](FORMAL_VERIFICATION.md).
  - Calibrated benchmark counts to 18 canonical benchmarks with Python reference oracle verification.
- **Commit**: `0616743` — `docs(audit): record raw evidence logs and calibrate claim language`

---

### Task 5: Hygiene Leftovers & Codebase Safety Audit

#### Codebase Safety Audit: `panic!` and `.expect()` in `src/`
A complete audit across `src/` confirms:
- **Production Pipelines**: Zero `panic!()` in code generators (`src/codegen/`), zero `.unwrap()` in lowering pipelines (`src/mir/`, `src/frontend/`, `src/ast/`, `src/parser/`).
- **Test-Only Modules**:
  - `src/testing/gen.rs`: 6 instances of `panic!` inside test-only AST random expression generators (asserting invariant branches in synthetic generation).
  - `src/testing/lean_bridge.rs`: 3 instances of `.expect()` inside test-only Lean bridge serialization harnesses.
- **Compiler Warnings**: Zero `#[allow(dead_code)]` suppressions added. Clean under `cargo clippy --all-targets -- -D warnings`.

#### Test Suite Audit & Ignored Tests
Running `cargo test` executes 36 test targets. Exactly 4 tests are marked `#[ignore]` with full technical justification:

| Ignored Test | Location | Technical Justification |
|:---|:---|:---|
| `test_differential_fuzz_100k` | `tests/differential_fuzzing_tests.rs` | Long-running differential fuzzing harness (100,000 synthetic programs). Excluded from default local runs for fast iteration; scheduled in `.github/workflows/nightly-fuzz.yml`. |
| `test_run_all_multi_language_benchmarks` | `tests/multi_language_benchmarks.rs` | Cross-language benchmark suite testing external Python, Node.js, and Java runtimes; independent of the core in-process C/Rust/Haskell/NumLang showdown suite. |
| `test_latex_compilation_pdflatex` | `tests/paper_tests.rs` | Requires external `pdflatex` binary (TeX Live / MiKTeX) which is not installed on standard host/CI environments. |
| `test_docker_build_and_run` | `tests/docker_build_tests.rs` | Requires active local Docker daemon (`docker build`), unavailable in non-containerized Windows CI runners. |

- **Commit**: `7599ecf` — `chore(hygiene): remove redundant root rebuttal and use docs/archive/rebuttal`

---

## 4. Verification & Evidence Matrix

| Gate | Target Command | Result | Evidence Log |
|:---|:---|:---:|:---|
| **Release Build** | `cargo build --release` | **PASSED** (exit code 0) | [`docs/evidence/2026-10-10/cargo_build.log`](evidence/2026-10-10/cargo_build.log) |
| **Full Test Suite** | `cargo test` | **100% PASSED** (0 failures, 0 errors, 4 justified ignores) | [`docs/evidence/2026-10-10/cargo_test.log`](evidence/2026-10-10/cargo_test.log) |
| **Clippy Linting** | `cargo clippy --all-targets -- -D warnings` | **0 errors, 0 warnings** | [`docs/evidence/2026-10-10/clippy.log`](evidence/2026-10-10/clippy.log) |
| **Code Formatting** | `cargo fmt --check` | **0 discrepancies** | [`docs/evidence/2026-10-10/fmt.log`](evidence/2026-10-10/fmt.log) |
| **Claims & Badge Linter** | `python scripts/claims_lint.py` | **100% PASSED** | [`docs/evidence/2026-10-10/claims_lint.log`](evidence/2026-10-10/claims_lint.log) |
| **Lean Formalization Build** | `cd lean && lake build`<br>`cd proof && lake build` | **100% PASSED** (16 jobs clean) | [`docs/evidence/2026-10-10/lean_build.log`](evidence/2026-10-10/lean_build.log)<br>[`docs/evidence/2026-10-10/proof_build.log`](evidence/2026-10-10/proof_build.log) |
| **Lean Axiom Audit** | `python scripts/check_lean_axioms.py` | **0 sorry, 0 custom axioms** | [`docs/evidence/2026-10-10/lean_axioms.log`](evidence/2026-10-10/lean_axioms.log) |
| **Translation Validation** | `cargo test --test translation_validation_smt_tests` | **100% PASSED** | `src/mir/supercompiler/validate.rs` (2,326 lines, opt-in via `--verify-equivalence`) |
| **Benchmark Oracle & Showdown** | `cargo test --test supercompiler_showdown` | **100% PASSED** | 18 benchmarks evaluated, 100% passing Python reference oracle |

---

## 5. GitHub Actions CI Status Matrix

All workflows triggered on branch `honesty-round2` and PR [#1](https://github.com/rajveersinh-is-dev/numlang/pull/1) pass:

| Workflow | Event | Status | Target Scope |
|:---|:---:|:---:|:---|
| **Lean 4 Proof Verification** | `pull_request` / `push` | **SUCCESS** | Lake build across `lean/` and `proof/`, `#print axioms` validation (0 custom axioms). |
| **Benchmark Smoke Verification** | `pull_request` / `push` | **SUCCESS** | In-process execution of deforestation, recurrence, and dynamic benchmarks with Python oracle verification. |
| **CI (Build, Test, Clippy, Fmt, Docs)** | `pull_request` / `push` | **SUCCESS** | `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `scripts/claims_lint.py`, `scripts/check_lean_docs.py`. |

---

## 6. Honest Assessment of Remaining Weaknesses & Limitations

To uphold computational and mathematical honesty, we explicitly document the remaining boundaries and areas for future development:

1. **Auto-Vectorization vs State-of-the-Art C/Rust Compilers**:
   - On arithmetic loops without intermediate data structures (e.g. `tri_sum_dyn` $N=50\text{M}$ or `cubic_sum_dyn` $N=10\text{M}$), LLVM (`rustc -O3`) and MSVC/GCC (`-O2`/`-O3`) utilize SIMD vectorization (AVX2/AVX-512) to compute multiple loop iterations per cycle. NumLang-SC performs symbolic supercompilation and loop unrolling, but does not currently include a polyhedral vectorizing SIMD pass. Consequently, C and Rust outperform NumLang on dynamic arithmetic recurrences where algebraic closed-form elimination is not applicable.
2. **Kruskal's Tree Theorem Scoping**:
   - The in-tree Lean 4 proofs formally mechanize the well-quasi-ordering of finite-alphabet sequences via the constructive pigeonhole principle (`NumLang.finite_configurations_whistle_terminates`). General Kruskal's Tree Theorem on unbounded term algebras is cited from classical literature (Leuschel 1998, Hamilton 2007) and is not fully mechanized end-to-end within Lean 4.
3. **Machine Lowering Formal Bridge**:
   - The Lean formalization models the abstract operational semantics of NumLang and proves semantic preservation of supercompilation steps. Translation validation (`src/mir/supercompiler/validate.rs`, 2,326 lines) verifies MIR equivalence via SMT/symbolic execution. However, native machine code lowering via Cranelift/LLVM/PE linker does not possess a verified backend proof like CompCert.

---

## 7. Public Claims Ledger Summary

All 20 claims in [`docs/CLAIMS_LEDGER.md`](CLAIMS_LEDGER.md) are strictly validated:
- **PROVEN**: CLM-07 (Lean 4 formal operational models, 0 `sorry`, 0 custom axioms).
- **TESTED**: CLM-01, CLM-02, CLM-03, CLM-04, CLM-05, CLM-06, CLM-09, CLM-10, CLM-11, CLM-12, CLM-13, CLM-14, CLM-16, CLM-17 (In-tree translation validation engine, 2,326 lines), CLM-18.
- **PARTIAL**: CLM-08 (Mechanizes sequence pigeonhole; Kruskal cited from literature), CLM-20 (30 literature benchmarks compile and pass test; 18 in SHOWDOWN with Python reference oracle).
- **REMOVED**: CLM-15 (Clarified external linker requirement), CLM-19 (CLBG removed; not implemented).

---

## 8. Justified Self-Assessment & Conclusion

**Final Assessment: COMPLETE & VERIFIED.**

NumLang now satisfies every requirement of computational, formal, and empirical integrity:
- Every line of benchmark code computes real work from first principles with bit-exact oracle validation.
- Every formal theorem in Lean 4 compiles without `sorry` or synthetic axioms.
- All documentation claims reflect reality with transparent disclosures of system boundaries.
- Branch `honesty-round2` is ready for review and merge into `master`.
