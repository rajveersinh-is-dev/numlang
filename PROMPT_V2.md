# PROMPT: Elevating NumLang to the World's Best General Optimizing Supercompiler (NumLang v2.0)

## Mission Directive

You are tasked with engineering **NumLang v2.0**, transforming NumLang from the world's most effective *numerical loop supercompiler* into the **world's premier general-purpose optimizing supercompiler**. 

While NumLang v1.0 dominates LLVM and GCC on closed-form algebraic induction, multi-variable recurrence acceleration, and modular loop collapse ($O(N) \to O(1)$), general-purpose dominance requires competing with and surpassing **LLVM 19+**, **GCC 14+**, **Rustc**, and academic supercompilers across **arbitrary systems software, pointer-heavy structures, memory-bound algorithms, and functional abstractions**.

---

## The 10-Phase Implementation Roadmap

### Phase 1: High-Level Mid-Level Intermediate Representation (MIR)
- **Problem**: AST-level supercompilation cannot easily reason about unstructured control flow, exceptions, or fine-grained pointer projections.
- **Deliverable**:
  - Implement `src/mir/`: A clean SSA-form control-flow graph (CFG) representation.
  - Basic blocks terminated by `Branch`, `Switch`, `Return`, or `Unreachable`.
  - Place-based projections (`x.field[i]`), rvalues, and operands.
  - Dominator tree computation (`src/mir/dominance.rs`) and loop nesting forest.
  - Verification: AST-to-MIR lowering pass with round-trip textual IR dumper (`numlang --emit-mir`).

### Phase 2: MemorySSA & Rigorous Alias Analysis
- **Problem**: In general software, memory stores through pointers/references hide opportunities for loop vectorization and closed-form derivation.
- **Deliverable**:
  - Implement `MemorySSA`: Versioned memory states (`MemoryDef`, `MemoryUse`, `MemoryPhi`).
  - Flow-sensitive points-to analysis distinguishing disjoint heap/stack allocations.
  - Aggressive `Mem2Reg`: Promotes stack allocations and struct fields to pure SSA registers.
  - Dead Store Elimination (DSE) and Redundant Load Elimination across function boundaries.

### Phase 3: Language Surface Expansion — Generics, Sum Types, & Slices
- **Problem**: General software requires polymorphic data structures and dynamic sequences.
- **Deliverable**:
  - **Generics**: Monomorphized type parameters `fn map<T, U>(val: T, f: fn(T) -> U) -> U` and `struct Pair<T, U> { first: T, second: U }`.
  - **Tagged Unions (Enums)**: `enum Option<T> { None, Some(T) }` with pattern matching.
  - **Slices & Dynamic Strings**: `&[T]` slice windows with fat-pointer representation `(ptr, len)` and zero-copy string views `&str`.
  - **C ABI Interoperability**: `extern "C" fn malloc(size: usize) -> *mut u8;` and C-compatible struct layouts (`#[repr(C)]`).

### Phase 4: Polyhedral Affine Loop Supercompiler (Surpassing LLVM Polly)
- **Problem**: Dense numerical computations that cannot be mathematically collapsed to $O(1)$ must be optimized for modern cache hierarchies.
- **Deliverable**:
  - Model nested affine loops as integer polyhedra using Presburger relations.
  - Automatically calculate optimal cache-line tiling factors, loop interchange, and loop skewing.
  - Eliminate loop-carried data hazards to enable lock-free parallel execution across multi-core systems.

### Phase 5: Automatic Superword-Level Parallelism (SLP) & Vectorizer
- **Problem**: Cranelift's scalar lowering leaves 8x to 16x hardware throughput on the table for non-supercompiled code.
- **Deliverable**:
  - Implement an SLP vectorizer in `src/opt/slp.rs`: Identifies parallel independent scalar operations and bundles them into SIMD registers (`x86_64 AVX2` / `AVX-512` / `aarch64 NEON`).
  - Emit vector intrinsics: `_mm256_fmadd_pd`, `_mm256_shuffle_epi8`, masked gathers/scatters.
  - Achieve 100% vector parity with Clang/Rustc on standard vector benchmarks (Matrix Multiply, Dot Product, Image Filtering).

### Phase 6: SMT-Driven Bitvector Superoptimizer (Souper-Grade Synthesis)
- **Problem**: Tricky bit-manipulation, hash functions, and cryptographic primitives contain redundant instructions that human heuristics miss.
- **Deliverable**:
  - Integrate an embedded SMT synthesizer module (Z3 / Bitwuzla binding or native bitvector term solver).
  - For any basic block with $\le 16$ bitwise instructions (`and`, `or`, `xor`, `shl`, `shr`, `popcnt`), synthesize the provably shortest sequence of machine instructions.
  - Synthesize branchless conditional selects to completely eliminate unpredictable CPU branch mispredictions.

### Phase 7: Unified Deforestation & Higher-Order Stream Fusion
- **Problem**: Functional abstractions (`map`, `filter`, `fold`, iterators) create allocations and closure indirections.
- **Deliverable**:
  - Implement generalized deforestation across higher-order functions.
  - Eradicate intermediate data structures: transform `list.map(f).filter(p).fold(0, g)` into a single tight machine loop with zero heap allocation.
  - Prove termination via homeomorphic embedding over MIR graphs.

### Phase 8: Dual Backend Strategy (JIT Velocity + LLVM/Cranelift Co-Generation)
- **Problem**: Developers want instant incremental compilation during development, but maximum LTO/PGO optimization in production.
- **Deliverable**:
  - **Development Mode (`--dev`)**: Blazing fast Cranelift backend (sub-10ms compilation for instant REPL/tests).
  - **Production Mode (`--release`)**: Direct LLVM / Cranelift-opt emitting full ThinLTO, Profile-Guided Optimization (PGO), and aggressive machine code placement.

### Phase 9: Self-Hosting Verification & Bootstrap
- **Problem**: A truly world-class general compiler must be capable of compiling itself.
- **Deliverable**:
  - Implement a complete NumLang parser and typechecker written in NumLang (`numlangc.nl`).
  - Compile `numlangc.nl` using the v1.0 Rust compiler to produce `numlangc_stage1.exe`.
  - Compile `numlangc.nl` using `numlangc_stage1.exe` to produce `numlangc_stage2.exe`.
  - Verify bit-for-bit identity between Stage 2 and Stage 3 (`diff stage2.exe stage3.exe` returns 0).

### Phase 10: The Global Benchmark Gauntlet
- **Problem**: Claims of "best in the world" must be supported by unimpeachable, independent data.
- **Deliverable**:
  - Execute NumLang v2.0 against the complete **Computer Language Benchmarks Game (CLBG)** and **PolyBench** suites against **Rust (`-O3`)**, **Clang/C (`-O3`)**, **GCC (`-O3`)**, and **GHC (`-O2`)**.
  - Pass 100% of functional correctness tests.
  - Win or tie on $\ge 90\%$ of benchmark workloads.

---

## Constraints & Execution Discipline

1. **Zero Cheats**: Every speedup must be earned through verifiable algebraic derivation, polyhedral scheduling, SMT synthesis, or vector lowering.
2. **Backward Compatibility**: All existing v1.0 NumLang programs and 36+ test suites must compile and pass without modification.
3. **Commit Cadence**: Commit cleanly after every individual phase with descriptive semantic commit messages.
4. **Compile-Time Guarantees**: Concrete evaluation passes must remain bounded. Do not allow compiler hangs or memory leaks.
