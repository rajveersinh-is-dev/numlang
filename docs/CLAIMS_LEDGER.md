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
| **CLM-01** | Global Hamilton distillation eliminates intermediate data structures in consumer-producer chains | `README.md:15`, `SHOWDOWN.md:12` | `tests/distillation_tests.rs`, `src/mir/supercompiler/distill.rs` | **TESTED** | Phase 3 |
| **CLM-02** | Multi-Result Supercompilation (MRSC) generates hypergraphs with Pareto selection | `README.md:16`, `ROADMAP.md:Phase 22` | `tests/mrsc_tests.rs`, `src/mir/supercompiler/mrsc.rs` | **TESTED** | a403bb8 |
| **CLM-03** | Constant forward differences and companion matrices collapse recurrences from $O(N)$ to $O(1)$ and $O(\log N)$ | `README.md:17`, `SHOWDOWN.md:13` | `tests/supercompiler_symbolic_tests.rs`, `src/mir/supercompiler/generalize.rs` | **TESTED** | a403bb8 |
| **CLM-04** | Reynolds defunctionalization converts higher-order closures to first-order dispatches | `README.md:18`, `LANGUAGE.md:8` | `tests/defunctionalization_tests.rs`, `src/opt/defunctionalize.rs` | **TESTED** | a403bb8 |
| **CLM-05** | Lazy thunk supercompilation collapses codata pipelines into register loops | `README.md:19`, `SHOWDOWN.md:55` | `tests/lazy_thunk_tests.rs`, `src/mir/supercompiler/drive.rs` | **TESTED** | a403bb8 |
| **CLM-06** | Self-applicable specializer `minspec.nl` structures Futamura 1, 2, and 3 projections | `README.md:20`, `ROADMAP.md:Phase 25` | `tests/third_futamura_tests.rs`, `src/bin/minspec_cogen.rs` | **TESTED** | a403bb8 |
| **CLM-07** | Lean 4 mechanized operational models have zero `sorry` and zero unproven axioms | `README.md:21`, `docs/FORMAL_VERIFICATION.md` | `lean/Supercompiler/`, `proof/NumLangProofs/`, CI `lean.yml` | **PARTIAL** (Standard Lean axioms: Classical.choice, propext, Quot.sound) | a403bb8 |
| **CLM-08** | Kruskal's Tree Theorem mechanization guarantees supercompiler termination | `docs/TERMINATION_PROOF.md:6` | `proof/NumLangProofs/Termination.lean:276-339` | **UNPROVEN** (Proves finite-alphabet pigeonhole; Kruskal cited from literature) | a403bb8 |
| **CLM-09** | NumLang supercompiler achieves 9 of 14 dominant wins against industrial compilers | `SHOWDOWN.md:84` | `tests/supercompiler_showdown.rs`, `SHOWDOWN.md:65-79` | **REMOVED** (Inflated: sub-timer floor, 0 ns Rustc, and regressions labeled wins) | a403bb8 |
| **CLM-10** | Peak recurrence collapse speedup of 125,682x on `tri_sum` | `SHOWDOWN.md:85` | `bench/showdown/numlang/tri_sum.nl` | **TESTED** (Algorithmic $O(N) \to O(1)$ loop collapse) | a403bb8 |
| **CLM-11** | Zero preloaded lookup tables, precomputed answers, or hardcoded shortcuts | `INTEGRITY_RULES.md:4` | `tests/integrity_lint.rs`, `src/mir/supercompiler/` | **TESTED** | Phase 3 |
| **CLM-12** | Algorithmic generality: compiler optimizations never match function or variable names | `INTEGRITY_RULES.md:20-22` | `tests/structural_generality_tests.rs`, `src/opt/recursion.rs` | **TESTED** | Phase 3 |
| **CLM-13** | Zero panic in codegen and lowering pipelines | `INTEGRITY_RULES.md:15` | `src/codegen/`, `src/mir/lower.rs` | **TESTED** (0 `panic!()`, 0 `.unwrap()` in lowering/codegen) | a403bb8 |
| **CLM-14** | Zero clippy warnings under `cargo clippy --all-targets -- -D warnings` | `INTEGRITY_RULES.md:17` | GitHub Actions CI `ci.yml` | **TESTED** (Passing with 0 warnings in CI) | a403bb8 |
| **CLM-15** | Pure Rust implementation | `Cargo.toml`, `README.md` | `src/`, `Cargo.toml` | **REMOVED** (Requires external native linker; optional LLVM backend is C++) | a403bb8 |
| **CLM-16** | Cross-platform execution across Linux, macOS, and Windows | `README.md`, `ROADMAP.md:Phase 41` | GitHub Actions CI matrix `[ubuntu, macos, windows]` | **TESTED** (All 3 platforms green in CI) | a403bb8 |
| **CLM-17** | Formal SMT-based translation validation via bit-vectors and Horn clauses | `ROADMAP.md:Phase 24` | `tests/translation_validation_smt_tests.rs`, `src/mir/supercompiler/validate.rs` | **TESTED** (In-tree QF_BV CDCL decision procedure) | a403bb8 |
| **CLM-18** | Two-level specialization disk cache (`.numlang_cache/`) | `ROADMAP.md:Phase 37` | `tests/supercompiler_phase37_tests.rs`, `src/mir/supercompiler/cache.rs` | **TESTED** | f9adfe8 |
| **CLM-19** | Computer Language Benchmarks Game (CLBG) evaluations (Phase 58) | `ROADMAP.md:Phase 58` | N/A | **REMOVED** (No CLBG benchmark implementation or data exists) | a403bb8 |
| **CLM-20** | 30 canonical literature benchmarks evaluated | `ROADMAP.md:Phase 39` | `tests/supercompiler_phase39_tests.rs`, `SHOWDOWN.md` | **PARTIAL** (30 benchmarks compile and pass correctness in test; 14 in SHOWDOWN) | a403bb8 |
