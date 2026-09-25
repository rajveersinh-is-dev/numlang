# Phase 20 Summary: Core Residualization, Back-Edge Knot Transfers & Textbook MSG

## Completed Tasks

1. **Parallel Copy Knot State Transfer (`src/mir/supercompiler/residualize.rs`)**:
   - Compared the active symbolic state environment $\sigma_{\text{curr}}$ at knot nodes (`ProcessEdge::Knot(anc_id)`) with the ancestor environment $\sigma_{\text{anc}}$.
   - Computed required variable state updates for all ancestor parameters and live registers.
   - Implemented parallel copy assignment generation using fresh scratch temporaries (`_knot_tmp_*`) to break cyclic data-flow dependencies and prevent variable clobbering before emitting the branch to the ancestor block.

2. **Residual Block ID Remapping for Phi Nodes (`src/mir/supercompiler/residualize.rs`)**:
   - Built a comprehensive predecessor mapping `node_preds: ProcessNodeId -> Vec<ProcessNodeId>` across tree edges, split transitions, and knot back-edges.
   - Constructed `phi_remap: (ProcessNodeId, BasicBlockId) -> BasicBlockId` linking original CFG basic block IDs to their residual predecessor block IDs.
   - Updated `SymTerm::Phi` evaluation in `emit_term_eval` to rewrite incoming branches to residual block IDs, eliminating mismatched/dangling predecessor references.

3. **General Place Evaluation & Memory Layout in Cranelift Backend (`src/codegen/cranelift_backend.rs`)**:
   - Implemented `get_place_value` handling arbitrary chained place projections (`Payload(idx)`, `Index(place)`, `Field(name)`, `Deref`).
   - Wired `get_place_value` across all statement assignments, rvalues (`BinaryOp`, `UnaryOp`, `Call`, `Array`, `Discriminant`, `EnumVariant`, `Alloc`, `Load`), and block terminators (`Return`, `BranchIf`, `Switch`).
   - Added `Rvalue::Discriminant`, `Rvalue::Alloc`, and `Rvalue::Load` codegen support in `compile_mir_function` with heap allocation via native `malloc` and byte-accurate layout copying.
   - Resolved the `0xc0000005` access violation root cause in recursive/heap-allocating functions.

4. **Textbook First-Order Anti-Unification (MSG) (`src/mir/supercompiler/generalize.rs`)**:
   - Implemented textbook first-order anti-unification (Plotkin 1970; Sørensen & Glück 1995) in `most_specific_generalization` and `anti_unify`.
   - Guaranteed variable sharing: identical subterm differences $(t_1, t_2)$ reuse the exact same generalized variable.
   - Supported recursive ADT constructors, unary/binary expressions, discriminant tags, calls, and conditionals.

5. **Integration & Test Verification**:
   - Verified that all heap-allocating and recursive benchmarks execute with zero crashes and valid exit codes:
     - `bench/numlang/nrev.nl` -> output: `12000`, exit code: `224` (`12000 % 256`)
     - `bench/numlang/append3.nl` -> output: `46500`, exit code: `164` (`46500 % 256`)
     - `bench/numlang/tree_flip.nl` -> output: `37600`, exit code: `224` (`37600 % 256`)
     - `bench/numlang/peano_mul.nl` -> output: `1200`, exit code: `176` (`1200 % 256`)
   - Added `tests/heap_supercompile_tests.rs` covering textbook MSG and end-to-end benchmark execution.
   - Re-enabled committing functions with tied knots in `src/mir/supercompiler/mod.rs`.
   - Full test suite verified green with 0 clippy warnings (`cargo clippy --all-targets -- -D warnings`).
