# Phase 2: Semantic Analysis & Static Type Checker - Context

**Gathered:** 2026-09-10
**Status:** Ready for planning
**Mode:** Smart discuss (accepted recommendations)

<domain>
## Phase Boundary

Implement symbol resolution, variable scope management, and strict static type checking for numlang.
</domain>

<decisions>
## Implementation Decisions

### Type System & Scoping
- **Default literal inference:** Integer literals default to `i64` and float literals default to `f64` when unannotated (`let x = 42;`).
- **Strict type coercion:** No implicit conversions between types (e.g., `f64 + i64` rejected at compile time) to guarantee numerical precision and avoid silent performance penalties.
- **Variable shadowing:** Block-scoped variable shadowing permitted (nested blocks can re-bind names).
- **Immutability defaults:** Variable bindings are immutable by default.

### the agent's Discretion
- Internal symbol table and type environment representation (stack of scopes / hash tables).
- Diagnostic error codes and helper notes for type mismatches.
</decisions>

<code_context>
## Existing Code Insights

- `src/ast.rs` defines `Expr`, `Stmt`, `Function`, `Literal`, `BinaryOp`.
- `src/span.rs` provides `Span` tracking source coordinates.
- `src/diagnostic.rs` formats errors using `miette`.
</code_context>

<specifics>
## Specific Ideas

- Symbol table should record declared functions and variables with their types and spans.
- Type checker pass validates that binary operations only operate on matching operands.
- Return statement type must match function signature.
</specifics>

<deferred>
## Deferred Ideas

- Generic types and user-defined structs (deferred to later milestones).
</deferred>
