# Phase 48: Total Algorithmic Generality & Structural Decoupling — Plan

> **Phase**: 48  
> **Status**: Planned (Awaiting User Signal to Execute)  
> **Traceability**: Master Plan §3, Requirements GEN-01..05  

## Objective
Purge all remaining string-matching heuristics (`ack`, `tak`, `append3`, struct field names `"x"`, `"y"`, etc.) from NumLang. Replace them with rigorous structural CFG/AST pattern matching ($S_3$ cyclic permutation groups with branch guards), bounded symbolic induction for nested deep recurrences, structural distillation triggers, and type-directed struct layout tables.

## Requirements
- **GEN-01**: Implement `detect_symmetric_permutation_recurrence` in `src/opt/recursion.rs` to detect 3-way cyclic argument permutations with decrements ($\pi_1=(x-1,y,z), \pi_2=(y-1,z,x), \pi_3=(z-1,x,y)$) and branch conditions ($x \le y$), contracting Takeuchi recurrences by structural induction regardless of function/variable names.
- **GEN-02**: Implement bounded symbolic induction for nested deep recurrences in `src/opt/recursion.rs`, specializing affine parameter slices ($m \in \{1, 2\}$) via symbolic driving and arithmetic progression detection to replace hardcoded Ackermann identities.
- **GEN-03**: Replace `func.name == "append3"` in `src/mir/supercompiler/mod.rs` and `lower.rs` with structural check `is_nested_recursive_composition(func)` and MIR `TransformIntent::Distill`.
- **GEN-04**: Replace hardcoded field name heuristics (`"x"`, `"y"`, `"z"`, `"first"`) in `src/codegen/llvm_backend.rs` with a type-directed `struct_layouts: HashMap<String, StructLayout>` populated directly from `Type::Struct`.
- **GEN-05**: Verify structural generality in `tests/structural_generality_tests.rs` using renamed/obfuscated symbols across all literature recurrence benchmarks.

## Verification
- Renaming benchmark functions (e.g. `ack` to `f_nested`, `tak` to `cyclic_3`) yields 100% identical optimizations and speedups.
- `cargo test --test structural_generality_tests` passes 100%.
- Zero occurrences of string-matching heuristics remain across the compiler.
