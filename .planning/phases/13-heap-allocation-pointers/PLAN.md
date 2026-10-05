# Phase 13: Heap Allocation (Box<T>, Deref) & Symbolic Pointer Driving — Plan

> **Phase**: 13
> **Status**: Completed
> **Traceability**: Master Plan Part II, Requirements HEAP-01..05
> **Milestone**: Core Supercompiler & Language Extensions (Phases 10–19)

## Objective
Add safe heap memory primitives (`Box<T>`, `box`, `deref`), dynamic allocation, and symbolic heap modeling within the supercompiler driver.

## Requirements
- **HEAP-01**: Type system extension `Type::Box(Box<Type>)` representing an owned heap allocation pointer.
- **HEAP-02**: Syntax and parsing for `box expr` allocation and `*expr` dereference expressions.
- **HEAP-03**: MIR statements for heap allocation (`Alloc`), load dereference (`Load`), and store (`Store`).
- **HEAP-04**: Symbolic heap state tracking in `src/mir/supercompiler/state.rs` mapping symbolic locations to term values.
- **HEAP-05**: Driving heap operations symbolically: folding store-load sequences and eliminating intermediate heap allocations.

## Key Deliverables
- `src/typecheck/types.rs`, `src/mir/lower.rs`, `src/mir/supercompiler/state.rs`, `src/mir/supercompiler/drive.rs`
- Test suite: `tests/heap_tests.rs`

## Verification
- Verified linked list and recursive tree construction, heap traversal, and symbolic pointer dereference.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
