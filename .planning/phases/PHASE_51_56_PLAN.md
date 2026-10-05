# Master Implementation Plan: Phases 51–56 (Total Unconditional Dominance)

> **Governing Documents**: [`INTEGRITY_RULES.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/INTEGRITY_RULES.md), [`ROADMAP.md`](file:///c:/Users/davea/.gemini/antigravity/scratch/numlang/ROADMAP.md)
> **Milestone**: Total Unconditional Dominance (Phases 51–56)
> **Status**: Planned (Awaiting User Signal to Execute)
> **Derived from**: Competitive gap analysis performed 2026-10-03

---

## 1. Executive Summary & Competitor Analysis

This milestone closes every remaining competitive loss domain identified after Phase 50. On completion, NumLang wins against **all** known supercompilers, JIT engines, and optimizing compilers across **all** evaluation dimensions.

| Competitor | Competitor Advantage | NumLang Root Cause of Loss | Dominance Strategy |
|:---|:---|:---|:---|
| **GHC Supercompiler** | Non-strict lazy codata streams | Strict SSA driving diverges on infinite codata | **Phase 51**: Thunk MIR extension + lazy driving + stream fusion |
| **GraalVM Truffle / V8** | Speculative deopt + OSR | Static single specialization, no runtime fallback | **Phase 52**: TypeGuard terminator + deopt stubs + OSR |
| **MRSC prototype** | Unbounded hypergraph IDDFS | Bounded whistle misses deep-optimal residuals | **Phase 53**: mrsc_oracle IDDFS + 4D Pareto cost model |
| **Hamilton Distiller** | Lambda-level fold/unfold on mutual HO recursion | Defunctionalization destroys lambda structure | **Phase 54**: Pre-defunc AST distillation in hodistill.rs |
| **LLVM Polly / Pluto** | ILP-solved polyhedral tiling + diamond schedules | No ILP solver, only basic fusion | **Phase 55**: Bareiss Simplex + Pluto constraints + tiling |
| **GCC -Os / LuaJIT** | Minimal binary; sub-ms cold startup | Specialization bloats binary; driving overhead | **Phase 56**: Block outliner + tiered JIT (Tier 0 <2ms, Tier 1 background) |

---

## 2. Traceability Matrix

| Requirement | Description | Phase | Deliverables |
|:---|:---|:---:|:---|
| LAZY-01 | Rvalue::Thunk and Terminator::Force MIR extensions | 51 | src/mir/mod.rs |
| LAZY-02 | Demand-propagation analysis | 51 | src/mir/thunk_analysis.rs |
| LAZY-03 | Lazy driving mode with SymTerm::Thunk | 51 | src/mir/supercompiler/drive.rs |
| LAZY-04 | Stream fusion over thunk producer-consumer chains | 51 | src/mir/supercompiler/distill.rs |
| LAZY-05 | Codata supercompilation verification suite | 51 | tests/codata_supercompilation_tests.rs |
| DEOPT-01 | Type-profile analysis at polymorphic call sites | 52 | src/mir/speculate.rs |
| DEOPT-02 | Terminator::TypeGuard fast-path emission | 52 | src/mir/mod.rs, residualize.rs |
| DEOPT-03 | Deoptimization stubs in Cranelift backend | 52 | src/codegen/cranelift/deopt.rs |
| DEOPT-04 | OSR entry points in Cranelift preambles | 52 | src/codegen/cranelift/mod.rs |
| DEOPT-05 | Speculative deopt verification suite | 52 | tests/speculative_deopt_tests.rs |
| ORACLE-01 | IDDFS oracle with --mrsc-exhaustive flag | 53 | src/mir/supercompiler/mrsc_oracle.rs |
| ORACLE-02 | MrscCostModel (4-dimensional) | 53 | src/mir/supercompiler/mrsc.rs |
| ORACLE-03 | Pareto frontier extraction from IDDFS residual set | 53 | src/mir/supercompiler/mrsc_oracle.rs |
| ORACLE-04 | L2 cache integration of IDDFS winners | 53 | src/mir/supercompiler/cache.rs |
| ORACLE-05 | Exhaustive MRSC oracle verification suite | 53 | tests/mrsc_oracle_tests.rs |
| HODIST-01 | Lambda-level process-tree distillation AST pass | 54 | src/ast/hodistill.rs |
| HODIST-02 | Inter-procedural Hamilton fold rule | 54 | src/ast/hodistill.rs |
| HODIST-03 | Higher-order deforestation of compose-chains | 54 | src/ast/hodistill.rs |
| HODIST-04 | Pipeline integration before lower_to_mir | 54 | src/compiler.rs |
| HODIST-05 | HO AST distillation verification suite | 54 | tests/ho_ast_distillation_tests.rs |
| ILP-01 | Fraction-free Bareiss Simplex for integer polyhedra | 55 | src/mir/supercompiler/polyhedral_ilp.rs |
| ILP-02 | Pluto permutability constraint encoding | 55 | src/mir/supercompiler/polyhedral_ilp.rs |
| ILP-03 | Rectangular loop tiling from ILP schedule | 55 | src/mir/supercompiler/polyhedral.rs |
| ILP-04 | Tiled vectorized loop nest emission in MIR | 55 | src/mir/supercompiler/polyhedral.rs |
| ILP-05 | Polyhedral ILP scheduler verification suite | 55 | tests/polyhedral_ilp_tests.rs |
| OUTLINE-01 | Content-addressed basic block sequence hasher | 56 | src/mir/supercompiler/outliner.rs |
| OUTLINE-02 | Duplicate block extraction into shared subroutines | 56 | src/mir/supercompiler/outliner.rs |
| OUTLINE-03 | Tiered compilation dispatch | 56 | src/compiler.rs |
| OUTLINE-04 | Background SupercompileWorker thread + atomic OSR | 56 | src/runtime/tier.rs |
| OUTLINE-05 | Tiered JIT and outlining verification suite | 56 | tests/tiered_jit_tests.rs |

---

## 3. Phase 51: Lazy/Thunk SSA Extension & Codata Supercompilation

### 3.1. Objectives & Scope
Close gap vs GHC Supercompiler. Extend MIR with explicit thunk representation. Implement demand-propagation analysis. Add lazy symbolic driving mode. Implement stream fusion over infinite codata producer-consumer chains without divergence.

### 3.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: Thunk Representation in SSA MIR
SSA is inherently eager. To model laziness in SSA we introduce:
- `Rvalue::Thunk { body: MirBodyId, env: Vec<LocalId> }`: Allocates a thunk closure.
- `Terminator::Force { thunk: LocalId, result: LocalId, cont: BasicBlockId }`: Forces a thunk on demand.

This mirrors GHC's STG machine thunk representation adapted to explicit SSA form.

#### Difficult Bit 2: Demand-Propagation Analysis
In `thunk_analysis.rs`, propagate demands backward through SSA use-def chains:
1. Initialize all thunks with demand Bottom (never demanded).
2. Walk forward: if a local is used in a non-thunk operation, mark as fully_demanded.
3. Propagate backward. Fully-demanded thunks on all paths are converted to eager computation.

#### Difficult Bit 3: Lazy Symbolic Driving Without Divergence
When driving a `Terminator::Force`:
1. Represent the thunk as `SymTerm::Thunk(MirBodyId, captured_sym_vals)`.
2. Only expand the thunk when a downstream use demands its value.
3. This prevents infinite unrolling of `iterate f x` before the consumer `take N` establishes a bound.

#### Difficult Bit 4: Stream Fusion
Given `take(N, zipWith(f, iterate(g, x), iterate(h, y)))`:
1. `iterate` produces a SymTerm::Thunk stream.
2. `zipWith` pairs thunk elements on demand.
3. `take(N, ...)` demands exactly N elements.
4. Distillation fuses all three into a single zero-allocation N-iteration loop.

### 3.3. Target Files
- `src/mir/mod.rs`, `src/mir/thunk_analysis.rs`
- `src/mir/supercompiler/drive.rs`, `src/mir/supercompiler/distill.rs`
- `tests/codata_supercompilation_tests.rs` (5 new tests)

### 3.4. Verification Gate
- `cargo test --test codata_supercompilation_tests` 100% green.
- `take(1000, zipWith(+, iterate(*2, 1), iterate(*3, 1)))` compiles to zero malloc calls.
- Output matches GHC; NumLang wall time beats GHC.

---

## 4. Phase 52: Speculative Type Guards & Deoptimization Safepoints

### 4.1. Objectives & Scope
Close gap vs GraalVM Truffle / V8. Enable speculative type-specialized compilation with runtime deoptimization fallback via TypeGuard terminators, Cranelift deopt stubs, and OSR entry points.

### 4.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: Type-Profile Analysis
In `speculate.rs`, during symbolic driving: for each polymorphic call site where the argument is `SymTerm::Unknown`, record `TypeProfile { local, observed_tag, confidence }`. Confidence = 1.0 for statically monomorphic, 0.0 for fully unknown.

#### Difficult Bit 2: TypeGuard Emission
For call sites where `confidence >= 0.95`, emit:
```
Terminator::TypeGuard { local, expected_tag, fast_path, deopt_stub }
```
- `fast_path`: the specialized block optimized for `expected_tag`.
- `deopt_stub`: block calling `__nl_deopt(deopt_id, live_vars...)`.

#### Difficult Bit 3: Deoptimization Frame Reconstruction
In `cranelift/deopt.rs`, implement `DeoptMetadata` mapping each TypeGuard site to its live register set. The runtime `__nl_deopt` function reconstructs an interpreter stack frame from register values and continues at the unspecialized function entry.

#### Difficult Bit 4: OSR Entry Points
In `cranelift/mod.rs`, Tier 1 functions emit an OSR preamble: check an `AtomicPtr` flag; if null, fall through to Tier 0 code. When Tier 1 compiles, the worker atomically sets the pointer to the T1 entry.

### 4.3. Target Files
- `src/mir/speculate.rs`, `src/mir/mod.rs`
- `src/codegen/cranelift/deopt.rs`, `src/codegen/cranelift/mod.rs`
- `tests/speculative_deopt_tests.rs` (5 new tests)

### 4.4. Verification Gate
- `cargo test --test speculative_deopt_tests` 100% green.
- Polymorphic function: 9,999/10,000 i64 calls take fast path; 1 f64 call correctly deoptimizes.
- Deoptimized output is bit-identical to unspecialized baseline.

---

## 5. Phase 53: Exhaustive MRSC Oracle with IDDFS & Pareto Cost Model

### 5.1. Objectives & Scope
Close gap vs Mitchell & Klyuchnikov's MRSC prototype. Implement unbounded IDDFS over the MRSC configuration hypergraph via `--mrsc-exhaustive`. Add 4-dimensional MrscCostModel and Pareto frontier extraction.

### 5.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: IDDFS over Residual Hypergraph
For depth limit d = 1, 2, 3, ...:
- At each node: try (a) continue driving, (b) fold with any ancestor, (c) MSG generalize.
- At depth d: if no complete residual found, increment d and retry.
- Under `--mrsc-exhaustive`: no whistle budget; terminates on full exploration or `--mrsc-timeout-secs`.

#### Difficult Bit 2: 4-Dimensional MrscCostModel
For each residual P, compute cost vector:
1. Step count (symbolic unrolling)
2. Allocation count (malloc calls)
3. Code size (basic block count)
4. Register pressure (max live vars)

#### Difficult Bit 3: Pareto Frontier Extraction
P dominates Q iff cost(P) <= cost(Q) componentwise and cost(P) != cost(Q). Extract the Pareto-dominant subset. Select winner via configurable linear combination `--mrsc-objective speed/size/balanced`.

### 5.3. Target Files
- `src/mir/supercompiler/mrsc_oracle.rs` (new), `src/mir/supercompiler/mrsc.rs`
- `src/mir/supercompiler/cache.rs`
- `tests/mrsc_oracle_tests.rs` (5 new tests)

### 5.4. Verification Gate
- `cargo test --test mrsc_oracle_tests` 100% green.
- IDDFS depth >= 20 finds strictly smaller residuals than bounded online MRSC on >= 3 benchmarks.
- Pareto selection is deterministic across repeated runs.

---

## 6. Phase 54: Pre-Defunctionalization Higher-Order AST Distillation

### 6.1. Objectives & Scope
Close gap vs Hamilton's Pure Distillation Engine. Implement Hamilton global process-tree distillation at the typed functional AST level before MIR lowering, preserving lambda term structure for fold/unfold rules on deeply composed HO chains.

### 6.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: Lambda-Level Process Tree
Model AST nodes as:
- `Config(Expr, Env)` -- driving configuration
- `Fold(ancestor)` -- back-edge (knot)
- `Branch(cond, Vec<child>)` -- case/if split
- `Result(Expr)` -- terminal reduced form

#### Difficult Bit 2: Hamilton Fold Rule
For new configuration C_new and ancestor C_anc:
1. Check alpha-equivalence: exists bijection rho on variable names such that C_new[rho] = C_anc.
2. If so, emit Fold(C_anc) and introduce generalization parameter capturing rho^{-1}.
3. This creates a back-edge yielding a loop in the distilled output.

#### Difficult Bit 3: Compose-Chain Deforestation
`compose f (compose g (compose h k))`:
1. Unfold compose applications.
2. Drive `f(g(h(k(x))))` chain.
3. Distillation fuses into single function `fused(x)` with no intermediate allocation.

#### Difficult Bit 4: Integration
In `compiler.rs`: insert `hodistill::distill_program(&mut ast)` after typechecking, before `lower_to_mir`. Controlled by `--ho-distill` (on by default in `--supercompile` mode).

### 6.3. Target Files
- `src/ast/hodistill.rs` (new), `src/compiler.rs`
- `tests/ho_ast_distillation_tests.rs` (5 new tests)

### 6.4. Verification Gate
- `cargo test --test ho_ast_distillation_tests` 100% green.
- 5-deep compose chain distills to single-pass application (AST node count reduced).
- Zero compose applications in distilled AST.

---

## 7. Phase 55: Pure-Rust Polyhedral ILP Scheduler (Pluto-style)

### 7.1. Objectives & Scope
Close gap vs LLVM Polly / Pluto / ISL. Implement fraction-free Bareiss Simplex over integer polyhedra. Encode Pluto permutability constraints. Compute legal multi-dimensional schedules enabling loop tiling and diamond tiling.

### 7.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: Fraction-Free Bareiss Simplex
Represent LP tableau as Vec<Vec<i128>>. Pivot using Bareiss formula:
```
T'[i][j] = (T[p][j] * T[i][j] - T[i][p_col] * T[p][j]) / prev_pivot
```
Integer division is exact by the Bareiss determinantal property. Terminates when all reduced costs >= 0 (optimal) or infeasibility detected.

#### Difficult Bit 2: Pluto Permutability Constraints
For each dependence between statements S_i, S_j with distance vector d:
- Emit: theta_i . d >= 0 as LP row.
- Solve for schedule coefficients theta via Simplex.

#### Difficult Bit 3: Rectangular Tiling
Apply T=32 rectangular tiling to permutable schedule:
- Outer tile loops: iterate over tile indices (ti, tj).
- Inner element loops: iterate over i in [ti*T, (ti+1)*T).
- LLVM auto-vectorizes the inner loop into AVX2 wide loads.

### 7.3. Target Files
- `src/mir/supercompiler/polyhedral_ilp.rs` (new)
- `src/mir/supercompiler/polyhedral.rs`
- `tests/polyhedral_ilp_tests.rs` (5 new tests)

### 7.4. Verification Gate
- `cargo test --test polyhedral_ilp_tests` 100% green.
- 3-nested matmul tiles legally under all dependences.
- Simplex terminates in <= 1,000 pivots for <= 8 loop dimensions.

---

## 8. Phase 56: Post-Residualization Outlining & Tiered JIT Compilation

### 8.1. Objectives & Scope
Close gap vs GCC -Os (binary size) and LuaJIT / V8 Sparkplug (cold latency). Content-addressed block outliner + two-tier JIT: Tier 0 (zero supercompilation, <2ms) + Tier 1 (background supercompilation, atomic OSR swap).

### 8.2. Theoretical Foundations & The Difficult Bits

#### Difficult Bit 1: Content-Addressed Block Hashing
Normalize each basic block: canonicalize LocalIds to %0, %1, ... in definition order. Compute SHA-256 over normalized opcode+type sequence. Group blocks with identical hashes as outline candidates.

#### Difficult Bit 2: Outline Extraction
For groups of >= 2 identical normalized blocks:
1. Compute live variable set at entry (union across all instances).
2. Extract to shared function `__nl_outlined_<hash[0..8]>(live_vars...) -> result`.
3. Replace all original sites with Terminator::Call to outlined function.
Target: binary size <= 120% of non-specialized baseline.

#### Difficult Bit 3: Two-Tier Architecture
In `compiler.rs`:
- Tier 0: MIR -> Cranelift, zero supercompilation. Insert per-function call counter.
- Tier 1: When counter > threshold (100), send (FunctionId, MirSnapshot) to SupercompileWorker.

#### Difficult Bit 4: Atomic OSR Function Pointer Swap
In `runtime/tier.rs`:
- Store each function entry as AtomicPtr<u8>.
- Tier 0 checks ptr: if non-null, jump to T1 code.
- SupercompileWorker stores T1 entry with AtomicPtr::store(t1_entry, Release).

### 8.3. Target Files
- `src/mir/supercompiler/outliner.rs` (new), `src/runtime/tier.rs` (new)
- `src/compiler.rs`
- `tests/tiered_jit_tests.rs` (5 new tests)

### 8.4. Verification Gate
- `cargo test --test tiered_jit_tests` 100% green.
- 10-function program compiles in Tier 0 in < 5ms cold.
- Tier 1 upgrade fires after 100 calls; optimized pointer observed in atomic slot.
- Outlined binary <= 120% of non-specialized baseline.
- Bit-identical outputs between Tier 0 and Tier 1.

---

## 9. Execution Protocol & Verification Summary

1. **Strict Sequential Execution**: Phases execute 51 -> 52 -> 53 -> 54 -> 55 -> 56. Each phase's verification gate must pass 100% before the next begins.
2. **Clippy Invariant**: `cargo clippy --all-targets -- -D warnings` must pass with 0 errors and 0 warnings throughout.
3. **No Benchmark Name Coupling**: All optimizations are structural and generic — zero string matches on function names or benchmark identifiers.
4. **No Preloaded Constants**: All algorithmic outputs computed dynamically from input program structure.
5. **Test Suite Continuity**: All existing test targets must remain green throughout all 6 phases.
