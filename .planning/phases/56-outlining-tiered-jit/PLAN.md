# Phase 56: Post-Residualization Outlining & Tiered JIT Compilation — Plan

> **Phase**: 56
> **Status**: Planned (Awaiting User Signal to Execute)
> **Traceability**: Master Plan §8, Requirements OUTLINE-01..05
> **Closes gap vs**: GCC -Os (binary size), LuaJIT / V8 Sparkplug (cold latency)

## Objective
Close the two remaining loss dimensions: (1) binary bloat from specialization via content-addressed block outlining, and (2) cold compilation latency via a two-tier JIT architecture (Tier 0: zero supercompilation <2ms; Tier 1: background supercompilation with atomic OSR function pointer swap).

## Root Cause of Loss (Binary Size)
Reynolds defunctionalization and MRSC residualization duplicate basic block sequences across specialization variants. On large programs, this causes NumLang binaries to be 2-3x larger than GCC -Os compiled equivalents.

## Root Cause of Loss (Cold Latency)
NumLang's supercompiler driving loop, whistle checks, and cache warmup take tens of milliseconds on cold runs. For short-lived CLI scripts where the entire execution completes in 2ms, this overhead dominates and negates all runtime gains — making LuaJIT's instant bytecode startup faster end-to-end.

## Requirements
- **OUTLINE-01**: Implement `src/mir/supercompiler/outliner.rs`: content-addressed hashing of normalized basic block sequences (opcodes + operand types, modulo register names via SHA-256). Detect duplicates with >= 90% sequence similarity.
- **OUTLINE-02**: Extract duplicated sequences into shared outlined subroutines `__nl_outlined_<hash[0..8]>` with live variable set as parameters. Replace original sites with `Terminator::Call`. Target: binary size <= 120% of non-specialized baseline.
- **OUTLINE-03**: Implement tiered compilation in `src/compiler.rs`: Tier 0 (MIR -> Cranelift, zero supercompilation, <2ms); per-function call counter; Tier 1 triggered at counter > 100.
- **OUTLINE-04**: Implement `src/runtime/tier.rs`: `SupercompileWorker` background thread; receives `(FunctionId, MirSnapshot)`; compiles with full supercompiler; atomically swaps function pointer via `AtomicPtr<u8>::store(t1_entry, Release)`.
- **OUTLINE-05**: Verify in `tests/tiered_jit_tests.rs` that: Tier 0 < 5ms cold, Tier 1 fires after hot threshold, outlined binary <= 120% baseline, Tier 0/Tier 1 outputs are bit-identical.

## Key Implementation Notes
- Normalization: canonicalize LocalIds to %0, %1, ... in definition order before hashing. Hash only opcode + operand type tags — NOT register indices.
- Atomic OSR: Tier 0 function preamble emits `br_if atomic_load(fn_ptr) != null, t1_entry`. Race-free because Tier 1 pointer is written with Release ordering and read with Acquire ordering.
- `SupercompileWorker` must respect all INTEGRITY_RULES: zero panic!(), zero .unwrap() in lowering pipeline.

## Verification
- `cargo test --test tiered_jit_tests` passes 100%.
- Tier 0 cold compilation of 10-function program completes in < 5ms.
- After 100 calls to a hot function, Tier 1 upgrade fires and the T1 pointer is observed in the atomic slot.
- Outlined binary <= 120% of non-specialized baseline binary size.
- All outputs are bit-identical between Tier 0 and Tier 1 execution.
- `cargo clippy --all-targets -- -D warnings`: 0 errors, 0 warnings.

