# Phase 6: Host CPU Architecture & SIMD Vectorization Engine - Context

**Gathered:** 2026-09-10
**Status:** Ready for planning
**Mode:** Autonomous (Phase 6 spec based on ROADMAP and project goals)

<domain>
## Phase Boundary

Specialize Cranelift backend to host CPU architecture with AVX2 and Fused Multiply-Add (FMA) instructions, accelerating mathematical and vector primitives.

Requirements: SIMD-01, SIMD-02, SIMD-03.
Success criteria:
1. Cranelift ISA detects and activates host features (`has_avx2`, `has_fma`, `has_sse42`, `has_bmi2`).
2. Built-in vector operations (`dot`, `vec_add`, `sum`) generate vectorized FMA assembly and unrolled execution.
3. Batch vector operations demonstrate significantly reduced cycle counts over scalar loops.

</domain>

<decisions>
## Implementation Decisions

### Host CPU Feature Detection
- Use `is_x86_feature_detected!("avx2")` and `is_x86_feature_detected!("fma")` at compiler runtime to dynamically query CPU capabilities.
- When supported, configure Cranelift `isa_builder.enable("has_avx2")` and `isa_builder.enable("has_fma")` so machine code generation utilizes full vector registers and fused operations.
- Provide a compiler flag `--target-cpu native` (or default to native) for optimal code generation on the host machine.

### FMA & Vector Math Acceleration
- Lower `dot(a, b)` using fused multiply-accumulate operations (`fma(a[i], b[i], acc)`) with multiple interleaved accumulators (`acc0`, `acc1`) to saturate CPU out-of-order execution pipelines and break dependency chains.
- Lower `vec_add(a, b)` and `sum(a)` using unrolled vector memory load/stores.
- Ensure strict floating-point correctness and exact numerical matching with IEEE-754 semantics.

</decisions>

<code_context>
## Existing Code Insights

- `src/codegen/cranelift_backend.rs` initializes Cranelift `isa_builder` and sets `opt_level = "speed"`.
- `compile_intrinsic_dot` unrolls dot product scalar loads and multiplies.
- `src/typecheck/checker.rs` validates array types and intrinsic calls.

</code_context>

<specifics>
## Specific Ideas

- Check Cranelift 0.135 `fma` instruction availability: `builder.ins().fma(a, b, acc)`.
- Use dual accumulators for dot product to achieve peak instruction-level parallelism (ILP).

</specifics>
