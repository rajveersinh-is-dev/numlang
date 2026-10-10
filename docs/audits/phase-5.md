# Phase 5 Self-Audit: Lean 4 Proofs and Axiom Inventory

Date: 2026-10-10  
Auditor / Orchestrator: Team Peer Review  
Status: **DONE**  

---

## 1. Hostile Re-read: The 5 Weakest Points & Attack Experiments

1. **Weak Point 1: Unsubstantiated "Kruskal's Tree Theorem Mechanized" Claims (F6, F8)**
   - *Attack*: Inspected `proof/NumLangProofs/Termination.lean:276-339` against the public claim in `docs/TERMINATION_PROOF.md`.
   - *Result*: `Termination.lean` implements `pigeonhole_seq` (the sequence pigeonhole principle for infinite sequences into finite lists) and proves `no_infinite_whistle_free_path` for finite alphabets. It does not mechanize Nash-Williams minimal bad sequence or Kruskal's full theorem for arbitrary unbounded term algebras.
   - *Resolution*: Clarified the exact mathematical boundaries in `docs/LEAN_STATUS.md` and `docs/TERMINATION_PROOF.md`. Formally acknowledged that `Termination.lean` mechanizes the finite-alphabet sequence pigeonhole termination theorem with 0 `sorry` and 0 domain axioms, while citing Kruskal's full theorem from the literature (Kruskal 1960, Leuschel 1998, Hamilton 2007). Updated `docs/CLAIMS_LEDGER.md` (CLM-08).

2. **Weak Point 2: Verification of Axiom Dependencies Across Both Packages**
   - *Attack*: Scanned all `.lean` files in `lean/` and `proof/` for unproven `axiom` declarations and admitted propositions (`sorry`).
   - *Result*: Grepped every file and ran `python scripts/claims_lint.py`. Zero `sorry` and zero custom `axiom` declarations exist.
   - *Resolution*: Documented the full standard axiom inventory in `docs/LEAN_STATUS.md`: proofs depend exclusively on standard Lean 4 core kernel axioms (`propext`, `Quot.sound`, `Classical.choice`), with 0 domain-specific axioms.

3. **Weak Point 3: Dual Lean Package Build Reproducibility**
   - *Attack*: Executed `lake build` independently in `lean/` and `proof/`.
   - *Result*: Both packages built with exit code 0 (`lean/`: 10 jobs completed; `proof/`: 6 jobs completed).
   - *Resolution*: Verified in-tree and in continuous integration (`.github/workflows/lean.yml`).

4. **Weak Point 4: Model Verification vs Native Binary Extraction**
   - *Attack*: Verified whether the Rust compiler executable is mechanically extracted from Lean 4.
   - *Result*: The Lean formalization models the abstract calculus and supercompilation algorithms. The Rust compiler binary is an independent systems implementation in Rust with Cranelift and LLVM codegen.
   - *Resolution*: Explicitly documented the boundary in `docs/FORMAL_VERIFICATION.md` and `docs/LEAN_STATUS.md`. The gap between Lean models and native execution is bridged by in-tree translation validation (`validate.rs` QF_BV SAT solver) and exhaustive differential testing.

5. **Weak Point 5: Translation Validation Engine Architecture Documentation**
   - *Attack*: Evaluated whether `validate.rs` is properly documented as an in-tree bit-blaster rather than an external dependency.
   - *Result*: Fully documented in `docs/FORMAL_VERIFICATION.md` and `docs/LEAN_STATUS.md`, detailing how the CDCL solver functions with zero C++ or Z3 runtime dependencies.

---

## 2. Verification Evidence

- `cd lean; lake build`: **PASSED** (10 jobs clean)
- `cd proof; lake build`: **PASSED** (6 jobs clean)
- `docs/LEAN_STATUS.md`: Created with complete theorem inventory and axiom disclosure
- `python scripts/claims_lint.py`: **PASSED** (100% claim integrity satisfied)
- `cargo clippy --all-targets -- -D warnings`: **PASSED** (0 errors, 0 warnings)

---

## 3. Assumptions, Guesses & Unverified Areas

- None. Both Lean packages compile cleanly from scratch.

---

## 4. Confidence Assessment

- **Axiom Purity**: **HIGH**. Zero `sorry`, zero domain axioms, standard Lean 4 kernel logic only.
- **Scientific Honesty**: **HIGH**. Clear distinction between mechanized finite-alphabet pigeonhole and literature Kruskal WQO citation.
