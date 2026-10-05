# Phase 12: Polymorphic Types & Generics with Monomorphization — Plan

> **Phase**: 12
> **Status**: Completed
> **Traceability**: Master Plan Part II, Requirements GENERIC-01..05
> **Milestone**: Core Supercompiler & Language Extensions (Phases 10–19)

## Objective
Implement generic functions and data structures with explicit type parameters `<T, U>`, supported by a whole-program monomorphization pass.

## Requirements
- **GENERIC-01**: AST syntax for generic parameters on functions (`fn id<T>(x: T) -> T`) and structs (`struct Pair<A, B>`).
- **GENERIC-02**: Representation of type variables (`Type::Param(String)`) in type definitions.
- **GENERIC-03**: Unification-based type inference for generic call sites.
- **GENERIC-04**: Whole-program monomorphization pass in `src/opt/monomorphize.rs` cloning and specializing generic functions for each concrete type combination.
- **GENERIC-05**: Clean Cranelift lowering receiving purely concrete, monomorphic types without boxing overhead.

## Key Deliverables
- `src/ast.rs`, `src/typecheck/types.rs`, `src/typecheck/checker.rs`, `src/opt/monomorphize.rs`
- Test suite: `tests/generics_tests.rs`

## Verification
- Verified generic containers, parametric identity, and map/fold functions across multiple types.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
