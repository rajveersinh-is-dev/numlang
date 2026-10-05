# Phase 54: Pre-Defunctionalization Higher-Order AST Distillation — Plan

> **Phase**: 54
> **Status**: Planned (Awaiting User Signal to Execute)
> **Traceability**: Master Plan §6, Requirements HODIST-01..05
> **Closes gap vs**: Hamilton's Pure Distillation Engine (Geoff Hamilton 2007)

## Objective
Implement Hamilton global process-tree distillation at the typed functional AST level — before MIR lowering and before defunctionalization — allowing Hamilton fold/unfold rules to operate on the full lambda term structure. This eliminates deeply composed higher-order chains and mutual HO recursion before the closure structure is destroyed by defunctionalization.

## Root Cause of Loss
Phase 49 defunctionalizes closures early into SSA discriminated unions. While this enables native codegen, it destroys the lambda term structure that Hamilton's fold/unfold rules operate on. Deep `compose`-chains and mutual HO recursion cannot be deforested once the closures are already flattened into switch statements.

## Requirements
- **HODIST-01**: Implement `src/ast/hodistill.rs`: a lambda-level global process-tree distillation pass. Model AST nodes as process tree configs: `Config(Expr, Env)`, `Fold(ancestor)`, `Branch`, `Result(Expr)`.
- **HODIST-02**: Implement Hamilton inter-procedural fold rule: check alpha-equivalence to any ancestor config (bijection rho on variables). If found, emit `Fold(ancestor)` and introduce generalization parameter.
- **HODIST-03**: Implement higher-order deforestation: unfold all `compose` applications and drive the resulting application chain to a single fused function via distillation.
- **HODIST-04**: Wire `hodistill::distill_program(&mut ast)` into `src/compiler.rs` before `lower_to_mir`, controlled by `--ho-distill` (default on in `--supercompile` mode).
- **HODIST-05**: Verify in `tests/ho_ast_distillation_tests.rs` that 5-deep compose chains fuse, mutual HO recursion distills to accumulator loops, and distilled AST contains zero compose applications.

## Verification
- `cargo test --test ho_ast_distillation_tests` passes 100%.
- 5-deep `compose` chain: AST node count reduced to a single fused application.
- Zero `compose` applications remain in the distilled AST.
- Mutual recursion across 3 HO functions distills into a single loop.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.
