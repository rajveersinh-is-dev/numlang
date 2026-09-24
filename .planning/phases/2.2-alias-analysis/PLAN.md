# Phase 2.2 Plan: Points-To & Field-Sensitive Alias Analysis

## Goal
Implement a precise, field-sensitive and array-index-aware alias analysis engine in `src/mir/alias.rs`. This provides static guarantees to prove disjointness between memory locations (`NoAlias`), exact identity (`MustAlias`), and potential aliasing (`MayAlias`), enabling individual struct fields and distinct local variables to be independently optimized and promoted.

## Tasks

1. **Design `AliasResult` and `AliasAnalysis` (`src/mir/alias.rs`)**:
   - `AliasResult`:
     - `NoAlias`: Memory locations are provably disjoint.
     - `MayAlias`: Memory locations might overlap.
     - `MustAlias`: Memory locations are provably identical.
   - `AliasAnalysis`:
     - Store points-to / base-allocation information for places in a function.
     - Provide `alias(&self, p1: &Place, p2: &Place) -> AliasResult`.
     - Provide `modref(&self, stmt: &Statement, place: &Place) -> ModRefResult` (useful for DSE / RLE).

2. **Core Disjointness Rules**:
   - **Root Places (Base Variables)**:
     - Distinct local variables / stack allocations (`x` vs `y` where `x != y` and neither is an alias of the other) $\implies$ `NoAlias`.
     - Distinct function parameters (pass-by-value in NumLang) $\implies$ `NoAlias`.
   - **Projections on Same Base**:
     - `p.x` vs `p.y` where field names differ $\implies$ `NoAlias` (Field Sensitivity).
     - `p.x` vs `p.x` $\implies$ `MustAlias`.
     - `arr[c1]` vs `arr[c2]` where indices are known distinct constants $\implies$ `NoAlias`.
     - `arr[c1]` vs `arr[c1]` with same constant $\implies$ `MustAlias`.
     - `arr[i]` vs `arr[j]` with unknown or differing dynamic indices $\implies$ `MayAlias`.
     - Sub-path matching: if `p1` is a prefix of `p2` (e.g. `p` vs `p.x`), `MayAlias` / partial overlap.
   - **Projections on Distinct Bases**:
     - If base of `p1` and base of `p2` are `NoAlias`, any projections on them (`p1.f` vs `p2.f`) are also `NoAlias`.

3. **Register Module in `src/mir/mod.rs`**:
   - `pub mod alias;`

4. **Unit Tests & Verification**:
   - Add unit tests in `tests/mir_tests.rs`:
     - Test distinct locals report `NoAlias`.
     - Test distinct struct fields (`p.x` vs `p.y`) report `NoAlias`.
     - Test same struct field (`p.x` vs `p.x`) reports `MustAlias`.
     - Test constant array indices (`arr[0]` vs `arr[1]`) report `NoAlias`.
     - Test identical constant array indices (`arr[2]` vs `arr[2]`) report `MustAlias`.
     - Test dynamic array indices (`arr[i]` vs `arr[j]`) report `MayAlias`.
   - Verify `cargo test --tests` passes and 0 clippy warnings.
