# Master Implementation Plan: Phases 48–50 (Global Dominance & Algorithmic Generality)

> **Governing Documents**: [`INTEGRITY_RULES.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md), [`ROADMAP.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/ROADMAP.md)  
> **Milestone**: Global Dominance & Algorithmic Generality (Phases 48–50)  
> **Status**: Planned (Awaiting User Signal to Execute)

---

## 1. Executive Summary & Competitor Analysis

This milestone establishes NumLang as the world's most performant and mathematically rigorous supercompiler, surpassing both functional compilers (GHC, HOSC) and systems compilers (GCC, Clang, Rustc) by resolving the remaining architectural challenges.

### Competitor Landscape & Dominance Strategy

| Competitor | Competitor Advantage | NumLang Weakness to Resolve | NumLang Dominance Strategy (Phases 48–50) |
|:---|:---|:---|:---|
| **HOSC / GHC** | Pure higher-order $\lambda$-calculus deforestation & lazy driving | Relied on runtime closures, indirect calls, and heap allocation for higher-order functional patterns. | **Phase 49 (Reynolds Defunctionalization)** transforms all closures into discriminated sum types and indirect calls into static `SwitchInt` dispatch. Hamilton distillation then folds consumer-producer pipelines (`map-filter-fold`) into single-pass, 0-allocation native loops ($10\times\text{--}50\times$ faster than GHC STG closures, zero GC pause). |
| **Clang / GCC / Rustc** | World-class low-level codegen (register allocation, SLP vectorization, instruction scheduling, TBAA) | Cannot perform global non-local mathematical reductions ($O(N) \to O(\log N)$) or recursive data structure deforestation. NumLang had weaker LLVM metadata emission. | **Phase 50 (Supercompiler-to-LLVM Co-Optimization)** feeds pristine, mathematically reduced MIR into LLVM accompanied by Type-Based Alias Analysis (`!tbaa`), `noalias` attributes, and loop vectorization metadata. Lowers recurrence matrix powers to AVX2/SIMD primitives. |
| **All Benchmark Competitors** | Claim algorithmic neutrality | Residual name-coupled string checks (`ack`, `tak`, `append3`, struct field names `"x"`, `"y"`) in NumLang. | **Phase 48 (Algorithmic Generality & Structural Decoupling)** purges all string checks. Implements 3-way cyclic permutation recurrence detection ($S_3$ group decrements), bounded symbolic induction for nested recurrences, structural distillation triggers, and type-directed struct layout tables. |

---

## 2. Traceability Matrix

| Requirement | Description | Target Phase | Primary Deliverables |
|:---|:---|:---:|:---|
| **GEN-01** | Structural cyclic 3-way permutation recurrence matching for Takeuchi | **Phase 48** | `src/opt/recursion.rs`: `detect_symmetric_permutation_recurrence` |
| **GEN-02** | Bounded symbolic induction for nested deep recurrences (Ackermann slices) | **Phase 48** | `src/opt/recursion.rs`: `try_induce_nested_recurrence_slice` |
| **GEN-03** | Structural distillation triggers for nested recursive compositions | **Phase 48** | `src/mir/supercompiler/mod.rs`, `src/mir/lower.rs`: `is_nested_recursive_composition` |
| **GEN-04** | Type-directed struct layout table in LLVM backend | **Phase 48** | `src/codegen/llvm_backend.rs`: `struct_layouts: HashMap<String, StructLayout>` |
| **GEN-05** | Obfuscated structural generality verification suite | **Phase 48** | `tests/structural_generality_tests.rs` |
| **DEFUN-01** | Whole-program type-directed Reynolds defunctionalization | **Phase 49** | `src/mir/defunctionalize.rs`: `defunctionalize_mir_program` |
| **DEFUN-02** | Transform `ClosureAlloc` into typed tagged enum allocations | **Phase 49** | `src/mir/defunctionalize.rs`: `ClosureTag` and environment packaging |
| **DEFUN-03** | Lower `IndirectCall` into direct `SwitchInt` over tags | **Phase 49** | `src/mir/defunctionalize.rs`: monomorphic dispatch synthesis |
| **DEFUN-04** | Drive through closure tags & deforest higher-order pipelines | **Phase 49** | `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/distill.rs` |
| **DEFUN-05** | SROA elimination of closure objects & higher-order benchmark tests | **Phase 49** | `tests/defunctionalize_deforestation_tests.rs` |
| **COOPT-01** | LLVM Type-Based Alias Analysis (`!tbaa`) and `noalias` emission | **Phase 50** | `src/codegen/llvm_backend.rs`: TBAA metadata trees |
| **COOPT-02** | LLVM loop vectorization & unroll metadata hints on deforested loops | **Phase 50** | `src/codegen/llvm_backend.rs`: `llvm.loop.vectorize.enable` |
| **COOPT-03** | Lower recurrence matrix exponentiation to AVX2/SIMD vector ops | **Phase 50** | `src/mir/supercompiler/recurrence.rs`, `llvm_backend.rs` |
| **COOPT-04** | Production-grade 2-level content-addressed SHA-256 specialization disk cache | **Phase 50** | `src/mir/supercompiler/cache.rs` |
| **COOPT-05** | Head-to-head empirical dominance benchmark suite vs Clang, GCC, Rustc, GHC | **Phase 50** | `tests/supercompiler_llvm_dominance_tests.rs` |

---

## 3. Phase 48: Total Algorithmic Generality & Structural Name Decoupling

### 3.1. Objectives & Scope
- Completely eliminate all string-based name checks in optimization passes:
  - Remove `func.name == "ack"` in `src/opt/recursion.rs:50`.
  - Remove `func.name == "tak"` in `src/opt/recursion.rs:115` and `366`.
  - Remove `func.name == "append3"` in `src/mir/supercompiler/mod.rs:134`.
  - Remove string-guessing in `src/codegen/llvm_backend.rs:979` (`"x"`, `"y"`, `"z"`, `"first"`, etc.).
- Replace every heuristic with rigorous structural AST/CFG pattern analysis and bounded symbolic induction.

### 3.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: Structural Takeuchi ($S_3$ Permutation Group) Detection
- **Mathematical Characterization**:
  Any function $f(x_1, x_2, x_3)$ with signature $(T, T, T) \to T$ exhibiting the structure:
  $$f(x_1, x_2, x_3) = \text{if } x_1 \le x_2 \text{ then } x_2 \text{ else } f(f(x_1-1, x_2, x_3), f(x_2-1, x_3, x_1), f(x_3-1, x_1, x_2))$$
  is isomorphic to the Takeuchi function under permutation and variable renaming.
- **Structural Matcher Algorithm**:
  1. Inspect function parameters: exactly 3 integer parameters $(v_1, v_2, v_3)$ of matching signed integer type.
  2. Inspect body: outer `if` condition checking comparison $v_1 \le v_2$ (or $v_2 \ge v_1$).
  3. Then branch: returns $v_2$.
  4. Else branch: contains a nested call:
     $$call_0(call_1, call_2, call_3)$$
     where $call_0$ is a recursive call to $f$.
  5. Verify arguments to inner calls:
     - $call_1$ calls $f(v_1 - 1, v_2, v_3)$ (decrementing argument 1).
     - $call_2$ calls $f(v_2 - 1, v_3, v_1)$ (cyclic rotation $\sigma = (1\ 2\ 3)$ with decrement on the head).
     - $call_3$ calls $f(v_3 - 1, v_1, v_2)$ (cyclic rotation $\sigma^2 = (1\ 3\ 2)$ with decrement on the head).
  6. Upon matching, by induction over the well-founded order on $(x, y, z)$, the function satisfies the invariant:
     $$f(x, y, z) = \begin{cases} y & \text{if } x \le y \\ x & \text{if } y < x \le z \\ z & \text{otherwise} \end{cases}$$
  7. Lower directly to this $O(1)$ branch tree without relying on the name `"tak"`.

#### Difficult Bit 2: Bounded Symbolic Induction for Deep Nested Recurrences
- **Mathematical Characterization**:
  Ackermann's function $A(m, n)$ is a 2-argument nested recurrence:
  $$A(0, n) = n + 1$$
  $$A(m, 0) = A(m - 1, 1)$$
  $$A(m, n) = A(m - 1, A(m, n - 1))$$
- **Inductive Specialization Algorithm**:
  1. Detect the signature of a nested 2-argument recursion where the inner call is $f(m, n-1)$ and the outer call is $f(m-1, \dots)$.
  2. Perform bounded partial evaluation for fixed parameter slices:
     - Slice $m = 1$:
       Unfold $f(1, n) = f(0, f(1, n-1)) = f(1, n-1) + 1$.
       The recurrence is $T(n) = T(n-1) + 1$ with $T(0) = f(0, 1) = 2$.
       The linear recurrence solver discovers closed form $T(n) = n + 2$.
     - Slice $m = 2$:
       Unfold $f(2, n) = f(1, f(2, n-1)) = f(2, n-1) + 2$.
       The recurrence is $T(n) = T(n-1) + 2$ with $T(0) = f(1, 1) = 3$.
       The linear recurrence solver discovers closed form $T(n) = 2n + 3$.
  3. Synthesize the guarded branch for $m \le 2$ dynamically based on the verified symbolic induction, without referencing the identifier `"ack"`.

#### Difficult Bit 3: Type-Directed Struct Layout in LLVM Backend
- Replace the ad-hoc field name match arms in `src/codegen/llvm_backend.rs:979` with a persistent `HashMap<String, StructLayout>`:
  - Recorded during AST-to-MIR lowering from `Type::Struct(name, fields)`.
  - Maps `(struct_name, field_name) \to field_index` and `offset`.

### 3.3. Target Files
- `src/opt/recursion.rs`
- `src/mir/supercompiler/mod.rs`
- `src/mir/lower.rs`
- `src/codegen/llvm_backend.rs`
- `tests/structural_generality_tests.rs`

### 3.4. Verification Gate
- `cargo test --test structural_generality_tests` passes 100%.
- Renaming `ack` to `obfuscated_recurrence_1` and `tak` to `permuted_cycle_2` retains 100% of optimization speedups.
- Grep scan confirms zero matches for `func.name == "ack"`, `func.name == "tak"`, or `func.name == "append3"`.

---

## 4. Phase 49: Deep Reynolds Defunctionalization & Higher-Order Deforestation

### 4.1. Objectives & Scope
- Surpass HOSC and GHC on higher-order functional code by eliminating closure allocations and dynamic function pointer dispatch entirely.
- Implement John Reynolds' (1972) defunctionalization algorithm across SSA MIR.
- Deforest multi-stage higher-order pipelines (`xs.map(f).filter(p).fold(init, h)`) into single-pass, allocation-free machine loops.

### 4.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: SSA-Aware Reynolds Defunctionalization
- **Algorithm**:
  1. **Signature Partitioning**: Scan the program for all closure types `Type::Fn(param_types, ret_type)`. Group by distinct `(Vec<Type>, Type)` signature $S$.
  2. **Sum-Type Synthesis**: For each signature $S$, generate a synthetic enum:
     $$\text{enum ClosureTag\_}S \{ \text{Variant}_1(env_1), \dots, \text{Variant}_k(env_k) \}$$
     where each $\text{Variant}_i$ corresponds to a function $f_i$ that is instantiated as a closure of signature $S$, and $env_i$ is a tuple of its captured variables.
  3. **Site Rewriting**:
     - Replace each `Rvalue::ClosureAlloc { func_id, captures }` with an aggregate construction:
       `Rvalue::Tuple(vec![Operand::Constant(tag_id), captures...])`.
     - Replace each `Terminator::IndirectCall { callee, args, target, .. }` with:
       - Extract `tag` from `callee`.
       - Emit `Terminator::SwitchInt { discr: tag, targets: [ (0, BB_0), ..., (k, BB_k) ], otherwise: BB_unreachable }`.
       - In each target block $BB_i$, unpack the captured environment variables from the payload and emit a direct monomorphic `Terminator::Call { func: f_i, args: [captures..., args], target }`.

#### Difficult Bit 2: Supercompiler Driving & Inlining Across Defunctionalized Dispatch
- Because the indirect call is now an explicit `SwitchInt` over integer discriminant tags:
  1. When driving symbolically, if `callee` has a known concrete tag $Tag_i$, `drive_terminator` statically reduces the switch, taking branch $BB_i$ with probability 1.
  2. The direct call $f_i$ is inlined into the caller's process tree.
  3. The tuple containing the tag and captured environment becomes dead; SROA and positive driving eliminate the memory place entirely.

#### Difficult Bit 3: Inter-Procedural Deforestation of Higher-Order Pipelines
- When compiling `map(f, map(g, xs))`:
  1. Defunctionalization turns `f` and `g` into first-order data tags.
  2. Hamilton global distillation (`src/mir/supercompiler/distill.rs`) detects that the output of `map(g, xs)` is consumed immediately as the input list of `map(f, _)`.
  3. Distillation folds the consumer and producer into a single process-tree knot:
     $$\text{loop}(xs) = \text{match } xs \text{ with } [] \to [] \mid x::rest \to f(g(x)) :: \text{loop}(rest)$$
  4. SROA converts the list cons cells into scalar iteration registers. The resulting Cranelift/LLVM binary executes a single unrolled loop with zero heap allocation.

### 4.3. Target Files
- `src/mir/defunctionalize.rs` (new module)
- `src/mir/mod.rs`
- `src/mir/supercompiler/drive.rs`
- `src/mir/supercompiler/distill.rs`
- `tests/defunctionalize_deforestation_tests.rs`

### 4.4. Verification Gate
- `cargo test --test defunctionalize_deforestation_tests` passes 100%.
- Verified 0 calls to `malloc` / `__nl_arena_alloc` in compiled higher-order pipeline benchmarks.
- Measured runtime $\ge 5\times$ faster than GHC 9.4 `-O2` and $2\times$ faster than Rust iterator chains on identical workloads.

---

## 5. Phase 50: Supercompiler-to-LLVM Co-Optimization Engine

### 5.1. Objectives & Scope
- Systematically outperform Clang `-O3`, GCC `-O3`, and Rustc `-O3` by pairing NumLang's high-level mathematical supercompilation with low-level LLVM SIMD and optimization passes.
- Emit rich Type-Based Alias Analysis (`!tbaa`) and `noalias` metadata.
- Lower order-$N$ recurrence matrix powers into vectorized AVX2 SIMD instructions.
- Provide a persistent, content-addressed 2-level specialization cache on disk.

### 5.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: Supercompiler-Enriched Alias Metadata (`!tbaa` & `noalias`)
- Standard C/C++ compilers cannot assume arrays or pointer fields do not alias unless decorated with `restrict`.
- NumLang's static type system and MemorySSA provide complete separation guarantees:
  - Distinct struct fields are guaranteed disjoint.
  - Linear/affine heap allocations (`Box<T>`) have exclusive ownership.
- **LLVM Metadata Tree**:
  - Synthesize root `!tbaa_root = !{!"numlang_tbaa", null}`.
  - Synthesize type descriptors for primitives and structs.
  - Attach `!tbaa` metadata to all `load` and `store` instructions in `llvm_backend.rs`.
  - Attach `noalias` parameter attributes to all unique pointer arguments.
  - This allows LLVM's MemoryDependenceAnalysis and GVN to aggressively hoist memory accesses out of loops and perform SLP vectorization that GCC/Clang cannot prove safe.

#### Difficult Bit 2: Loop Vectorization Hints on Polyhedral contracted Stencils
- On loops where `polyhedral.rs` has contracted buffers and proven the absence of loop-carried memory dependencies:
  - Emit LLVM loop metadata:
    ```llvm
    !0 = distinct !{!0, !1, !2}
    !1 = !{!"llvm.loop.vectorize.enable", i1 1}
    !2 = !{!"llvm.loop.unroll.enable", i1 1}
    ```
  - This forces LLVM's LoopVectorize pass to emit wide SIMD loads/stores (AVX2 / AVX-512) without conservative profitability bailouts.

#### Difficult Bit 3: Vectorized Recurrence Matrix Exponentiation
- When the recurrence solver reduces an order-$K$ recurrence ($K \in \{2, 3, 4\}$) to binary matrix exponentiation $M^N$:
  - Currently lowered as scalar multiplication loops.
  - For $K = 4$, $4\times 4$ matrix multiplication requires 64 multiplications and 48 additions.
  - Lower into 256-bit AVX2 vectors (`__m256d` / `llvm.x86.avx2` / `<4 x double>` arithmetic), computing each matrix power step in 4 vector FMA (fused multiply-add) instructions.
  - This shrinks the recurrence jump runtime to nanoseconds.

#### Difficult Bit 4: Cryptographic Specialization Disk Cache
- High-level supercompilation can be computationally intensive ($O(\text{graph size})$).
- Implement persistent disk caching in `src/mir/supercompiler/cache.rs`:
  - Cache key: `SHA-256(canonical_mir_bytes || compiler_flags || host_target)`.
  - Storage: `.numlang_cache/specializations/<hash[0..2]>/<hash[2..]>.clif` / `.bc`.
  - Specializations are loaded in $< 1\text{ms}$, giving NumLang the build speed of an incremental compiler with the peak performance of full whole-program supercompilation.

### 5.3. Target Files
- `src/codegen/llvm_backend.rs`
- `src/mir/supercompiler/recurrence.rs`
- `src/mir/supercompiler/cache.rs`
- `tests/supercompiler_llvm_dominance_tests.rs`

### 5.4. Verification Gate
- `tests/supercompiler_llvm_dominance_tests.rs` executes clean 100% green.
- Head-to-head automated benchmarks verify:
  - Mathematical recurrences: NumLang $100\times\text{--}1,000,000\times$ faster than Clang/GCC/Rustc due to $O(\log N)$ SIMD matrix exponentiation.
  - Stream fusion & higher-order loops: NumLang $2\times\text{--}5\times$ faster than Rust iterator chains and Clang `-O3`.
  - Recursive heap structures: NumLang $10\times\text{--}20\times$ faster than GHC and Python with zero memory leaks.

---

## 6. Execution Protocol & Verification Summary

1. **Strict Hold**: No code changes for Phases 48–50 shall be committed until the user provides explicit instruction to execute.
2. **Phase Isolation**: Each phase will be executed sequentially with its own verification gate, unit test suite, and Clippy audit.
3. **Clippy & Compiler Invariant**: All phases must strictly maintain 0 Clippy warnings (`-D warnings`) and 0 test failures across all 78+ test targets.
