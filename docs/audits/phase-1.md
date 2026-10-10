# Phase 1 Self-Audit: Enforcement Instead of Promises

Date: 2026-10-10  
Auditor / Orchestrator: Team Peer Review  
Status: **DONE**  

---

## 1. Hostile Re-read: The 5 Weakest Points & Attack Experiments

1. **Weak Point 1: Unsafe Code in Runtime Allocator vs `#![deny(unsafe_code)]`**
   - *Attack*: Added `#![deny(unsafe_code)]` to `src/lib.rs` to enforce the zero-unsafe invariant.
   - *Result*: The low-level runtime bump allocator (`src/runtime/arena.rs`), parallel worker pool (`src/runtime/parallel.rs`), and deoptimization FFI callback (`src/codegen/cranelift/deopt.rs`) interact with raw pointers and the OS C-ABI.
   - *Resolution*: Scoped `#![allow(unsafe_code)]` strictly to `src/runtime/mod.rs` and the single FFI export `__nl_deopt` in `deopt.rs`, accompanied by mandatory `// SAFETY:` justifications. The rest of the compiler remains under `#![deny(unsafe_code)]`.

2. **Weak Point 2: Intra-Doc Rustdoc Link Failures**
   - *Attack*: Ran `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items`.
   - *Result*: Rustdoc flagged 14 broken intra-doc link errors in `polyhedral_ilp.rs` (math expressions like `[j]`, `[k]`, `[p][q]`), `recurrence.rs` (`A[i][j]`), and unclosed HTML tags in `types.rs` (`Box<T>`).
   - *Resolution*: Enclosed all math and type parameters in backticks to prevent rustdoc from misinterpreting mathematical bracket indices as intra-doc links. Docs now build with 0 warnings under `-D warnings`.

3. **Weak Point 3: Proptest in Production Dependencies**
   - *Attack*: Inspected `Cargo.toml` dependency tree. `proptest = "1.11.0"` was pulled into standard runtime builds.
   - *Result*: `src/testing/gen.rs` imported `proptest` at module top-level for an unused strategy `random_program_strategy`.
   - *Resolution*: Gated `proptest` in `src/testing/gen.rs` under `#[cfg(test)]` and moved `proptest` exclusively into `[dev-dependencies]`.

4. **Weak Point 4: Metamorphic Transformation Verification**
   - *Attack*: Created `tests/metamorphic_anti_cheat_tests.rs` with alpha-renamed functions and variables (`zebra_kernel_accumulator`, `omega_bound`, `dynamic_second_order_eval`).
   - *Result*: The supercompiler successfully collapsed the renamed loops to identical closed forms without looking up names, proving that recurrence solving is purely structural.

5. **Weak Point 5: Packaging Hygiene & Bloat**
   - *Attack*: Ran `cargo package --allow-dirty --list`.
   - *Result*: Without package excludes, non-essential development directories (`paper/`, `rebuttal/`, `scratch/`, `.planning/`, `graphify-out/`) would be packaged into `.crate` archives.
   - *Resolution*: Added `exclude = ["paper/", "rebuttal/", "scratch/", ".planning/", "graphify-out/"]` to `Cargo.toml`.

---

## 2. Claims Linter & CI Verification

- `python scripts/claims_lint.py`:
  - [1/3] Local Markdown Links: **PASSED** (0 broken links)
  - [2/3] Live CI Status Badges: **PASSED** (0 static badges)
  - [3/3] Lean 4 Formalization Integrity: **PASSED** (0 `sorry`, 0 `axiom`)
- Documentation Build: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passed with 0 warnings.
- Metamorphic Anti-Cheat Suite: `cargo test --test metamorphic_anti_cheat_tests` passed (5/5 tests).

---

## 3. Assumptions, Guesses & Unverified Areas

- None in this phase. All enforcement mechanisms are mechanically checked by compiler flags, CI workflow steps, and dedicated test binaries.

---

## 4. Temptations & Shortcuts Avoided

- *Temptation*: To use global `#![allow(rustdoc::broken_intra_doc_links)]` instead of fixing doc comments.
  - *Reality*: Manually inspected and fixed all 14 doc-link sites in `polyhedral_ilp.rs`, `recurrence.rs`, and `types.rs`.
- *Temptation*: To retain `proptest` in `[dependencies]` to avoid touching `src/testing/gen.rs`.
  - *Reality*: Properly isolated `proptest` behind `#[cfg(test)]` and moved it into `[dev-dependencies]`.

---

## 5. Confidence Assessment

- **Enforcement Mechanics**: **HIGH**. Automated under `-D warnings` in both clippy and rustdoc.
- **Metamorphic Suite**: **HIGH**. Prevents name-matching regressions mechanically.

---

## 6. Regression Check

- `cargo fmt --check`: **PASSED**
- `cargo clippy --all-targets -- -D warnings`: **PASSED**
- `cargo test --test metamorphic_anti_cheat_tests`: **PASSED**
