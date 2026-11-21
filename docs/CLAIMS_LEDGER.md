# NumLang Public Claims Ledger

Maintained under Global Rule G3. Every public claim made in documentation, paper, benchmarks, or roadmap is tracked here with concrete verification evidence.

Status legend:
- **PROVEN**: Mechanically verified by formal proof (Lean 4 or SMT bit-vector decision procedure).
- **TESTED**: Verified by automated test suites or reproducible CI benchmarks.
- **PARTIAL**: Partially implemented or valid under restricted configurations.
- **UNPROVEN**: Claim made without sufficient mathematical proof or benchmark evidence.
- **REMOVED**: Overclaim or incorrect statement removed or downgraded.

---

| Claim ID | Public Claim | Stated Location | Concrete Evidence / Test Path | Status | Last Verified Commit |
|:---|:---|:---|:---|:---:|:---:|
| **CLM-01** | Global Hamilton distillation eliminates intermediate data structures in consumer-producer chains | `README.md:15`, `SHOWDOWN.md:12` | `tests/distillation_tests.rs`, `src/mir/supercompiler/distill.rs` | **TESTED** | 0aa6825 |
| **CLM-02** | Multi-Result Supercompilation (MRSC) generates hypergraphs with Pareto selection | `README.md:16`, `ROADMAP.md:Phase 22` | `tests/mrsc_lattice_tests.rs`, `tests/mrsc_oracle_tests.rs`, `src/mir/supercompiler/mrsc.rs` | **TESTED** | 0aa6825 |
| **CLM-03** | Constant forward differences and companion matrices collapse recurrences from $O(N)$ to $O(1)$ and $O(\log N)$ | `README.md:17`, `SHOWDOWN.md:13` | `tests/supercompiler_symbolic_tests.rs`, `src/mir/supercompiler/generalize.rs` | **TESTED** | 0aa6825 |
| **CLM-04** | Reynolds defunctionalization converts higher-order closures to first-order dispatches | `README.md:18`, `LANGUAGE.md:8` | `tests/defunctionalize_deforestation_tests.rs`, `tests/higher_order_tests.rs`, `src/mir/defunctionalize.rs` | **TESTED** | 0aa6825 |
| **CLM-05** | Lazy thunk supercompilation collapses codata pipelines into register loops | `README.md:19`, `SHOWDOWN.md:55` | `tests/codata_supercompilation_tests.rs`, `src/mir/supercompiler/drive.rs` | **TESTED** | 0aa6825 |
| **CLM-06** | Self-applicable specializer `minspec.nl` structures Futamura 1, 2, and 3 projections | `README.md:20`, `ROADMAP.md:Phase 25` | `tests/third_futamura_tests.rs`, `src/bin/minspec_cogen.rs` | **TESTED** | 0aa6825 |
| **CLM-07** | Lean 4 mechanized operational models have zero `sorry` and zero unproven axioms | `README.md:21`, `docs/LEAN_STATUS.md` | `lean/Supercompiler/`, `proof/NumLangProofs/`, `docs/LEAN_STATUS.md` | **PROVEN (abstract model only)** | 0aa6825 |
| **CLM-08** | Process tree termination formalization via homeomorphic embedding and sequence pigeonhole | `docs/TERMINATION_PROOF.md:6`, `docs/LEAN_STATUS.md` | `proof/NumLangProofs/Termination.lean:276-339`, `docs/LEAN_STATUS.md` | **PARTIAL** (Mechanizes finite-alphabet pigeonhole; Kruskal cited from literature) | 0aa6825 |
| **CLM-09** | NumLang supercompiler achieves 7 outright wins, 15 ties, and 0 regressions out of 22 benchmarks against industrial compilers | `SHOWDOWN.md:88`, `SHOWDOWN.md:89` | `bench/data/showdown_results.csv`, `scripts/make_bench_table.py` | **TESTED** (7 outright wins, 15 ties, 0 regressions vs Base out of 22 benchmarks under strict 95% CI rule) | e2b129c |
| **CLM-10** | Peak recurrence collapse speedup of 2168.84x on `tri_sum` (Euler closed form) | `SHOWDOWN.md:71`, `README.md:122` | `bench/data/showdown_results.csv`, `bench/showdown/numlang/tri_sum.nl` | **TESTED** (Algorithmic $O(N) \to O(1)$ loop collapse: 2168.84x [1785.27-2582.94]) | e2b129c |
| **CLM-11** | Zero preloaded lookup tables, precomputed answers, or hardcoded shortcuts | `INTEGRITY_RULES.md:4` | `tests/integrity_lint.rs`, `src/mir/supercompiler/` | **TESTED** | 0aa6825 |
| **CLM-12** | Algorithmic generality: compiler optimizations never match function or variable names | `INTEGRITY_RULES.md:20-22` | `tests/structural_generality_tests.rs`, `src/opt/recursion.rs` | **TESTED** | 0aa6825 |
| **CLM-13** | Zero panic in codegen and lowering pipelines | `INTEGRITY_RULES.md:15` | `src/codegen/`, `src/mir/lower.rs` | **TESTED** (0 `panic!()`, 0 `.unwrap()` in lowering/codegen) | 0aa6825 |
| **CLM-14** | Zero clippy warnings under `cargo clippy --all-targets -- -D warnings` | `INTEGRITY_RULES.md:17` | `.github/workflows/ci.yml` | **TESTED** (Passing with 0 warnings in CI) | 0aa6825 |
| **CLM-15** | Pure Rust implementation | `Cargo.toml`, `README.md` | `src/`, `Cargo.toml` | **REMOVED** (Requires external native linker; optional LLVM backend is C++) | 0aa6825 |
| **CLM-16** | Cross-platform execution across Linux, macOS, and Windows | `README.md`, `ROADMAP.md:Phase 41` | GitHub Actions CI matrix `[ubuntu, macos, windows]` | **TESTED** (All 3 platforms green in CI) | 0aa6825 |
| **CLM-17** | Formal SMT-based translation validation via bit-vectors and Horn clauses | `ROADMAP.md:Phase 24` | `tests/translation_validation_smt_tests.rs`, `src/mir/supercompiler/validate.rs` | **TESTED** (In-tree QF_BV CDCL decision procedure) | 0aa6825 |
| **CLM-18** | Two-level specialization disk cache (`.numlang_cache/`) | `ROADMAP.md:Phase 37` | `tests/supercompiler_phase37_tests.rs`, `src/mir/supercompiler/cache.rs` | **TESTED** | 0aa6825 |
| **CLM-19** | Computer Language Benchmarks Game (CLBG) evaluations (Phase 58) | `ROADMAP.md:Phase 58` | N/A | **REMOVED** (No CLBG benchmark implementation or data exists) | 0aa6825 |
| **CLM-20** | 30 canonical literature benchmarks evaluated | `ROADMAP.md:Phase 39` | `tests/supercompiler_phase39_tests.rs`, `SHOWDOWN.md` | **PARTIAL** (30 benchmarks compile and pass correctness in test; 18 in SHOWDOWN with Python reference oracle) | 0aa6825 |
