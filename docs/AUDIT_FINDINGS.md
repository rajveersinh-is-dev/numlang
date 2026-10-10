# NumLang Hostile Peer Review & Verification Audit Findings

Date: 2026-10-10  
Orchestrator & Hostile Auditor  
Repository: https://github.com/rajveersinh-is-dev/numlang  

---

## Executive Summary of Candidate Findings (F1 – F12)

| ID | Hypothesis Summary | Verification Status | Primary File:Line Evidence |
|:---|:---|:---:|:---|
| **F1** | Supercompiler is 0.59x–0.87x slower than baseline on deforestation benchmarks | **CONFIRMED** | [`SHOWDOWN.md:65-69`](../SHOWDOWN.md#L65-L69) |
| **F2** | "9 of 14 wins" count is inflated; timer floor (< 500 ns), 0 ns Rustc labeled NumLang wins; 1.00x tie | **CONFIRMED** | [`SHOWDOWN.md:67-75`](../SHOWDOWN.md#L67-L75) |
| **F3** | Static badge, no CI, no `.github/` directory | **REFUTED AT HEAD** (Confirmed historically) | [`.github/workflows/ci.yml:10-39`](../.github/workflows/ci.yml#L10-L39), [`README.md:5-7`](../README.md#L5-L7) |
| **F4** | `INTEGRITY_RULES.md` cited in README but missing from root | **REFUTED AT HEAD** (Confirmed historically) | [`INTEGRITY_RULES.md:1-22`](../INTEGRITY_RULES.md#L1-L22), [`README.md:140`](../README.md#L140) |
| **F5** | Benchmark-specific / name-matching optimizations existed in history; rule 4 not mechanically enforced | **CONFIRMED** | Git commit `3abe601`, [`ROADMAP.md:84`](../ROADMAP.md#L84), [`src/mir/supercompiler/distill.rs:942-964`](../src/mir/supercompiler/distill.rs#L942-L964), [`tests/integrity_lint.rs:34-42`](../tests/integrity_lint.rs#L34-L42) |
| **F6** | Naming mismatches: `cubic_sum` is quadratic ($i^2$); `tri_sum` has no modulus in code but SHOWDOWN claims mod $10^9+7$ | **CONFIRMED** | [`bench/showdown/numlang/cubic_sum.nl:5`](../bench/showdown/numlang/cubic_sum.nl#L5), [`bench/showdown/numlang/tri_sum.nl:5-12`](../bench/showdown/numlang/tri_sum.nl#L5-L12), [`SHOWDOWN.md:48-49`](../SHOWDOWN.md#L48-L49), [`README.md:120`](../README.md#L120) |
| **F7** | Benchmark fairness: Rustc Box vs NumLang leaky malloc; MSVC /arch:AVX2 vs Rustc -O; compilers NOT_INSTALLED; 14 vs 30 benchmarks; no variance/CI | **CONFIRMED** | [`bench/showdown/rust/nrev.rs:9-41`](../bench/showdown/rust/nrev.rs#L9-L41), [`src/codegen/cranelift/intrinsics.rs:215-231`](../src/codegen/cranelift/intrinsics.rs#L215-L231), [`tests/supercompiler_showdown.rs:60-95`](../tests/supercompiler_showdown.rs#L60-L95), [`SHOWDOWN.md:22-32,63-79`](../SHOWDOWN.md#L63-L79) |
| **F8** | Lean claims overstated: "zero unproven axioms" ignores standard Lean axioms; Kruskal's Tree Theorem claim is actually finite-list pigeonhole; no link from Lean model to Rust code | **CONFIRMED** | [`proof/NumLangProofs/Termination.lean:276-339`](../proof/NumLangProofs/Termination.lean#L276-L339), [`docs/TERMINATION_PROOF.md:6`](TERMINATION_PROOF.md#L6), [`README.md:21`](../README.md#L21) |
| **F9** | "Only known open-source system..." superlative is unverifiable | **CONFIRMED HISTORICALLY / REMOVED AT HEAD** | Git commit `6471ea5` vs `93556fe`, [`README.md`](../README.md) |
| **F10** | Overflow before division in closed forms ($N(N+1)/2$ and Faulhaber); cache key missing compiler version/flags; SMT validator undocumented | **CONFIRMED** | [`src/mir/supercompiler/generalize.rs:201-206,247-249`](../src/mir/supercompiler/generalize.rs#L201-L206), [`src/mir/supercompiler/cache.rs:27-35`](../src/mir/supercompiler/cache.rs#L27-L35), [`src/mir/supercompiler/validate.rs:1-60`](../src/mir/supercompiler/validate.rs#L1-L60) |
| **F11** | Packaging inconsistencies: pure Rust false; inkwell 0.5 with llvm18-0 vs LLVM 18/19; rust-version 1.82 vs >=1.80; proptest in dependencies; no lints table; both bench/ and benches/, lean/ and proof/; missing SECURITY.md | **CONFIRMED** | [`Cargo.toml:12,26,30`](../Cargo.toml#L12), [`README.md`](../README.md) |
| **F12** | Language incomplete: no syntax for closures or generics in reference; no memory deallocation model; undefined overflow behavior; Phase 58 (CLBG) has no results | **CONFIRMED** | [`LANGUAGE.md:1-250`](../LANGUAGE.md#L1-L250), [`ROADMAP.md:84-110`](../ROADMAP.md#L84-L110) |

---

## Detailed Evidence & Audit Notes

### F1: Deforestation Performance Regressions
- **Evidence**: [`SHOWDOWN.md:65-69`](../SHOWDOWN.md#L65-L69)
  - `nrev`: NumLang-SC 264.90 µs vs NumLang-Base 156.00 µs (0.59x speedup, i.e. 41% slowdown).
  - `append3`: NumLang-SC 104.70 µs vs NumLang-Base 68.40 µs (0.65x speedup, 35% slowdown).
  - `kmp`: NumLang-SC 40.70 µs vs NumLang-Base 31.00 µs (0.76x speedup, 24% slowdown).
  - `peano_mul`: NumLang-SC 38.00 µs vs NumLang-Base 33.00 µs (0.87x speedup, 13% slowdown).
  - `tree_flip`: NumLang-SC 112.10 µs vs NumLang-Base 67.30 µs (0.60x speedup, 40% slowdown).
- **Finding**: On every algebraic deforestation benchmark, supercompiling introduces additional intermediate residual overhead or misses full defunctionalization, performing strictly worse than the unoptimized baseline compiler.
- **Action Required**: Enforce a strict profitability gate: if the cost model or residual does not eliminate allocations/traversals, fall back to baseline. Under no circumstances may an optimizing compiler emit residuals slower than baseline.

### F2: Inflation of Showdown "Wins"
- **Evidence**: [`SHOWDOWN.md:67-75`](../SHOWDOWN.md#L67-L75)
  - `tri_sum`: NumLang-SC 100 ns, Rustc-O 0 ns. NumLang marked as Winner.
  - `cubic_sum`: NumLang-SC 100 ns, Rustc-O 0 ns. NumLang marked as Winner.
  - `compose5`: NumLang-SC 100 ns, Rustc-O 0 ns. NumLang marked as Winner.
  - `pow2_mod`: NumLang-SC 100 ns, NumLang-Base 100 ns, Rustc-O 0 ns. NumLang marked as Winner with a 1.00x speedup.
  - `kmp`, `peano_mul`, `tree_flip`: NumLang-SC was slower than NumLang-Base, yet NumLang-SC is labeled "Winner".
- **Finding**: The "9 of 14 wins" claim is mathematically false. Sub-timer floor measurements (< 500 ns / 0 ns) cannot be ranked, and slower residuals cannot be labeled winners.
- **Action Required**: Adopt an objective definition of "Win". Mark sub-timer measurements as "Instantaneous / Sub-timer resolution" rather than numerical speedups. Delete false winner designations.

### F3: Static Badge & Missing CI
- **Evidence**: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and [`README.md:5-6`](../README.md#L5-L6)
  - Active GitHub Actions workflow runs on every push across Ubuntu, macOS, and Windows.
  - Badges in `README.md` are live SVG badges pointing to GitHub Actions workflows.
- **Finding**: Refuted at current HEAD (the CI workflow is active and passing).

### F4: Missing INTEGRITY_RULES.md
- **Evidence**: [`INTEGRITY_RULES.md`](../INTEGRITY_RULES.md) is present at repo root (2,500 bytes) and committed.
- **Finding**: Refuted at current HEAD.

### F5: Benchmark Name Coupling & Integrity Rule 4
- **Evidence**:
  - [`src/mir/supercompiler/distill.rs:942-964`](../src/mir/supercompiler/distill.rs#L942-L964) hardcodes `GlobalTerm::Call { func: "append".to_string(), ... }` in `synthesize_append3`.
  - [`tests/integrity_lint.rs:34-42`](../tests/integrity_lint.rs#L34-L42) only checks exact string equality patterns like `== "append"` and `== "ack"`, failing to catch structural synthesis hardcoded to specific benchmark functions.
  - Git history shows Phase 29 explicitly targeted `Ackermann`, `stream_fusion`, and `fib_matrix`.
- **Action Required**: Replace hardcoded synthetic process tree templates with purely inductive, AST-agnostic transformation rules. Add metamorphic anti-cheat testing where benchmark identifiers are randomized.

### F6: Naming and Modulus Discrepancies
- **Evidence**:
  - [`bench/showdown/numlang/cubic_sum.nl:5`](../bench/showdown/numlang/cubic_sum.nl#L5): `acc = acc + (i * i);` is a sum of squares ($\sum i^2$), NOT cubic ($\sum i^3$).
  - [`bench/showdown/numlang/tri_sum.nl:5-12`](../bench/showdown/numlang/tri_sum.nl#L5-L12): Computes unbounded 64-bit integer sum and does `% 256` on exit; no `mod 10^9+7` exists anywhere in the code.
  - [`SHOWDOWN.md:48-49`](../SHOWDOWN.md#L48-L49): Claims $\sum i \pmod{10^9+7}$ and $\sum i^2 \pmod{10^9+7}$.
- **Action Required**: Either rename `cubic_sum` to `square_sum` or update its implementation to actually compute $i^3$. Synchronize SHOWDOWN table descriptions with actual benchmark code.

### F7: Benchmark Methodology & Fairness
- **Evidence**:
  - `nrev.rs` allocates and deallocates separate heap `Box<List>` objects on every recursive call, triggering thousands of system heap allocations/deallocations.
  - `nrev.nl` calls `__nl_malloc` and NEVER frees memory, running as an infinite bump leak.
  - MSVC receives `/O2 /arch:AVX2` while Rustc receives default `-O` (opt-level=2 without native vector extensions).
  - GHC, HOSC, SPSC, Clang, GCC are marked `NOT_INSTALLED`.
  - Phase 39 claimed 30 benchmarks, but only 14 are in SHOWDOWN.
- **Action Required**: Standardize compiler flags across all compilers (e.g. `-O3` / `-C opt-level=3` / `target-cpu=native`). Equalize allocation semantics or benchmark non-leaking arena variants. Document exact CPU, OS, and toolchain versions.

### F8: Lean 4 Proof Verification Claims
- **Evidence**:
  - [`docs/TERMINATION_PROOF.md:6`](TERMINATION_PROOF.md#L6) claims: "The homeomorphic embedding relation forms a Well-Quasi-Ordering via Kruskal's Tree Theorem".
  - [`proof/NumLangProofs/Termination.lean:276-339`](../proof/NumLangProofs/Termination.lean#L276-L339) proves only that an infinite path over a *pre-existing finite list of expressions* has a duplicate via the Pigeonhole Principle. It does NOT prove Kruskal's Tree Theorem over growing terms.
  - Proofs formalize an idealized inductive calculus in Lean, with no mechanization connecting the Lean AST to Rust MIR.
  - Uses standard Lean axioms (`Classical.choice`, `propext`, `Quot.sound`).
- **Action Required**: Clarify in docs that Kruskal's Tree Theorem is a mathematical property cited from the literature (Leuschel 1998, Turchin 1986), while the in-repo Lean proof mechanizes finite-alphabet pigeonhole termination and small-step big-step soundness. Document standard Lean axioms explicitly.

### F9: Unverifiable Superlatives
- **Evidence**: "Only known open-source system implementing the full chain" was present in early commits (`6471ea5`) and removed in `93556fe`.
- **Action Required**: Ensure all documentation remains free of unverifiable superlatives.

### F10: Soundness Bugs in Recurrence Closed Forms
- **Evidence**:
  - [`src/mir/supercompiler/generalize.rs:201-206`](../src/mir/supercompiler/generalize.rs#L201-L206): Computes $k \times (k - 1) / 2$ as `intern_binary(Mul, num_iters, k_minus_1)` followed by `intern_binary(Div, ..., 2)`.
  - For $k > 3 \times 10^9$, signed i64 multiplication overflows before division by 2.
  - [`src/mir/supercompiler/cache.rs:27-35`](../src/mir/supercompiler/cache.rs#L27-L35): `CacheKey` omits compiler version and optimization flags.
  - In-tree SMT/CDCL solver in [`src/mir/supercompiler/validate.rs`](../src/mir/supercompiler/validate.rs) is complete (2,326 lines) but undocumented in user guides.
- **Action Required**: Compute closed forms safely by factoring out parity: if $k$ is even, $(k / 2) \times (k - 1)$; if odd, $k \times ((k - 1) / 2)$. Add compiler version and flags to `CacheKey`.

### F11: Packaging & Metadata Inconsistencies
- **Evidence**:
  - External linker is required; optional LLVM backend is C++.
  - `inkwell = { version = "0.5.0", features = ["llvm18-0"] }` contradicts "LLVM 18/19".
  - `rust-version = "1.82.0"` in `Cargo.toml:12` vs README `>=1.80`.
  - `proptest = "1.11.0"` in `[dependencies]` instead of `[dev-dependencies]`.
  - Missing `#![deny(clippy::unwrap_used)]` in library crate root.
  - Both `bench/` and `benches/` exist; both `lean/` and `proof/` exist.
  - Missing `SECURITY.md`.
- **Action Required**: Move proptest to dev-dependencies, align rust-version, consolidate directories, add `[lints]` table with denial of unwrap and panic, add `SECURITY.md`.

### F12: Language Completeness & Roadmap Gaps
- **Evidence**:
  - `LANGUAGE.md` does not document closure syntax, generics syntax, or memory deallocation model.
  - Integer overflow model is unspecified.
  - Phase 58 (CLBG) has no benchmark entries or data.
- **Action Required**: Create formal `docs/SPEC.md` and update `LANGUAGE.md` with full grammar, types, integer overflow model, and memory model. Update `ROADMAP.md` to reflect true status of all phases.
