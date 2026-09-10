# Phase 7: Static Bounds Analysis, BCE & Loop Unrolling Pass - Context

**Gathered:** 2026-09-10
**Status:** Ready for planning
**Mode:** Autonomous (Phase 7 spec based on ROADMAP and project goals)

<domain>
## Phase Boundary

Eliminate runtime bounds checking overhead in proven loops and unroll tight fixed-iteration numerical loops.

Requirements: OPT-01, OPT-02, OPT-03.
Success criteria:
1. Static analyzer identifies loop induction variables and provable array index bounds `0 <= i < len`.
2. Bounds check elimination (BCE) omits runtime `icmp_imm_u` and panic branches in verified loop bodies and constant accesses.
3. Small fixed-size array loops and vector kernels are unrolled to saturate the CPU execution pipeline.

</domain>

<decisions>
## Implementation Decisions

### Static Induction & Range Analysis (OPT-01)
- Identify canonical loop induction patterns: `let mut i: i64 = <start>; while i < <limit> { ... i = i + 1; }`.
- When `start >= 0` and `limit <= arr_len`, annotate indexing `arr[i]` as provably safe (`is_safe = true`).
- Constant literal indices `arr[c]` where `0 <= c < arr_len` are also statically proven safe (`is_safe = true`).

### Bounds Check Elimination (BCE) (OPT-02)
- Add `is_safe: bool` flag to `TypedExpr::Index` and `TypedStmt::IndexAssign`.
- In Cranelift backend, check `if !is_safe { self.emit_bounds_check(...); }`.
- When `is_safe` is true, omit the `icmp_imm_u` comparison and panic block branch, emitting only address computation and load/store.

### Loop Optimization (OPT-03)
- Enable Cranelift mid-end loop invariant code motion (LICM) and dead code elimination (DCE).
- Unroll tight fixed loops in vector kernels.

</decisions>
