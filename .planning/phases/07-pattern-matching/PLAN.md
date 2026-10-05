# Phase 7: Pattern Matching (Match Expressions) — Plan

> **Phase**: 07
> **Status**: Completed
> **Traceability**: Master Plan Part I, Requirements MATCH-01..05
> **Milestone**: Production Compiler Baseline (Phases 1–9)

## Objective
Introduce first-class pattern matching over integers and booleans with exhaustive checking and or-patterns (`0 | 1 => ...`).

## Requirements
- **MATCH-01**: Lexer tokens: `Match`, `FatArrow` (`=>`), `Pipe` (`|`), and `Underscore` (`_`).
- **MATCH-02**: AST representation for `Expr::Match` with pattern arms supporting literals, wildcards, and or-patterns.
- **MATCH-03**: Semantic analysis checking scrutinee type against pattern types and enforcing expression type equality across all arm bodies.
- **MATCH-04**: Exhaustiveness checker verifying that boolean matches cover both `true` and `false` or include a wildcard arm.
- **MATCH-05**: Codegen lowering match expressions to multi-way conditional jump ladders or switch tables with phi-merge nodes for result values.

## Key Deliverables
- `src/token.rs`, `src/ast.rs`, `src/parser/expr.rs`, `src/typecheck/checker.rs`, `src/codegen/cranelift/`
- Test suite: `tests/match_tests.rs`

## Verification
- Verified literal matching, wildcard fallthrough, or-patterns, and nested match expressions.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
