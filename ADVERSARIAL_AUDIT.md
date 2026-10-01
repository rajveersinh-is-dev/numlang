# NumLang Adversarial Audit: Critical Vulnerabilities, Soundness Gaps & Engineering Traps

> **Audit Date**: October 1, 2026  
> **Target**: NumLang Supercompiler & Runtime Pipeline (v0.1.0, commit `77b0e7a`)  
> **Posture**: Deep Adversarial PL & Systems Review  
> **Verdict**: Highly innovative architecture, but undermined by formal verification tautologies, broken cross-platform compilation, unbounded memory leaks, and pseudo-self-applicable Futamura projections.

---

## Executive Summary

NumLang presents itself as an academic breakthrough: a formally verified (Lean 4), self-applicable (3 Futamura projections), SSA-based process-tree supercompiler that achieves $O(N) \to O(1)$ closed-form derivations and outperforms C, Rust, and Haskell across 30 benchmarks.

However, an aggressive, forensic inspection of the codebase reveals **critical vulnerabilities, theoretical sleights of hand, and architectural compromises** that would lead to immediate rejection by top-tier systems/PL referees (PLDI, POPL, ICFP, ASPLOS) or fatal crashes in production:

1. **The Lean 4 Soundness Proof is Vacuous**: Every major transformation theorem in `lean/Supercompiler/` (`Main.lean`, `Distillation.lean`, `MRSC.lean`, `Compaction.lean`) defines its inductive premise as `SemanticEquivalent f1 f2`. The proofs are circular tautologies ($A \to A$) that bypass actual compiler transformations.
2. **Disconnected Operational Semantics**: In `Semantics.lean`, `Evaluates` does not even invoke the `Step` relation. It merely inspects the initial environment map!
3. **The Futamura Projection Illusion**: In `src/stdlib/minspec.nl`, `first_futamura`, `second_futamura_compiler`, and `third_futamura_cogen` are **byte-for-byte identical functions**. `MinSpec` never specializes itself; it merely specializes a static toy AST interpreter.
4. **Unbounded Memory Leaks (Zero Deallocation)**: Heap pointers, recursive ADTs, boxes, and closures call Win32 `LocalAlloc` or `malloc` with **no `free`**, no garbage collection, and no RAII/drop semantics.
5. **Hardcoded Windows Kernel32 Symbols Break Linux/Docker**: Both Cranelift and LLVM backends unconditionally emit Win32 imports (`ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`), breaking linking on Linux and inside the official Docker reproduction container.
6. **Scalability & Stack Collapse**: Compiling a 30-line microbenchmark takes up to **28 seconds** in debug mode. The compiler requires a hardcoded **16 MB worker stack** to prevent stack overflow crashes.
7. **Monolithic Code Sprawl**: `src/codegen/cranelift_backend.rs` has ballooned to **9,306 lines (444 KB)**, containing two completely separate codegen pipelines (AST and MIR) with over 200 unchecked `.unwrap()` calls.

---

## 1. Formal Verification Façades (Lean 4)

### 1.1 The Circular Tautology Pattern ($A \to A$)
The paper and documentation boast:
> *"End-to-end soundness theorem (Main.lean) verified with zero unproven axioms and zero sorry."*

The reason there are zero axioms and zero `sorry` is that **the theorems are tautologies by construction**.

#### Evidence in `lean/Supercompiler/Main.lean`:
```lean
inductive SupercompilerProduces : MirProgram → MirProgram → Prop where
  | pipeline (prog residual : MirProgram) :
      (∀ f1 ∈ prog.functions, ∀ f2 ∈ residual.functions, f1.name = f2.name → SemanticEquivalent f1 f2) →
      SupercompilerProduces prog residual

theorem supercompiler_sound
    (prog : MirProgram) (residual : MirProgram)
    (h : SupercompilerProduces prog residual) :
    ∀ func residual_func,
      (func ∈ prog.functions) →
      (residual_func ∈ residual.functions) →
      func.name = residual_func.name →
      SemanticEquivalent func residual_func := by
  exact compose_all_proofs h
```
**Analysis**:
`SupercompilerProduces prog residual` is defined by constructor `pipeline`, which **requires as a prerequisite premise** that all matching functions are already semantically equivalent. The theorem `supercompiler_sound` does nothing more than unpack this premise. The actual Rust supercompiler algorithms (driving, whistle, MSG, distillation, recurrence solving) are **never defined as computational functions in Lean**.

#### Evidence in `lean/Supercompiler/Distillation.lean`:
```lean
inductive FoldStep : MirFunction → MirFunction → Prop where
  | fold (f1 f2 : MirFunction) :
      SemanticEquivalent f1 f2 →
      FoldStep f1 f2
```
A "fold step" is defined as a transition between two functions that are *already* semantically equivalent. Proving that a sequence of fold steps preserves semantics is trivial induction over a reflexive-transitive closure where every step is an equivalence by definition.

#### Evidence in `lean/Supercompiler/Compaction.lean`:
```lean
inductive NoopRemoval : MirFunction → MirFunction → Prop where
  | remove_nop (f1 f2 : MirFunction) :
      SemanticEquivalent f1 f2 →
      NoopRemoval f1 f2

theorem noop_removal_preserves_semantics (func func' : MirFunction) (h : NoopRemoval func func') :
    SemanticEquivalent func func' := by
  cases h with
  | remove_nop eq => exact eq
```
Every single compaction pass in `Compaction.lean` requires `SemanticEquivalent` as its constructor argument.

---

### 1.2 Disconnected Semantics: `Evaluates` Ignores `Step`
In `lean/Supercompiler/Semantics.lean`, the small-step transition relation `Step fn s s'` is defined across lines 80–151. However, look at how `Evaluates` is defined (lines 152–156):

```lean
inductive Evaluates : MirFunction → MirEnv → Val → Prop where
  | base (fn : MirFunction) (env : MirEnv) (retVar : Local) (v : Val) :
      lookup env retVar = some v →
      Evaluates fn env v
```
**Analysis**:
`Evaluates` does **not** assert `Step* s_init s_final`. It simply checks if the variable `retVar` is present in the initial environment `env`!
Because `SemanticEquivalent f1 f2` is defined as:
```lean
def SemanticEquivalent (f1 f2 : MirFunction) : Prop :=
  ∀ args result, Evaluates f1 args result ↔ Evaluates f2 args result
```
Any two functions that return a local with the same identifier name are **trivially semantically equivalent**, even if function `f1` computes `x + 1` and function `f2` formats the hard drive!

---

### 1.3 Pseudo-Multithreading in `Semantics.lean`
In `lean/Supercompiler/Semantics.lean` (lines 146–150):
```lean
  | fork (s : MirState) (b : MirBasicBlock) (left right join : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      b.term = Terminator.Fork left right join →
      Step fn s { s with pc := join, stmtIdx := 0 }
```
**Analysis**:
`Terminator::Fork left right join` is claimed to model thread-level parallel execution. But in the formal semantics, `Fork` simply jumps straight to `join`, completely skipping both `left` and `right` subtrees!

---

## 2. The Futamura Projection Façade

`ROADMAP.md` Phase 25 and `INTEGRITY_RULES.md` §5 claim to implement genuine, self-applicable 2nd and 3rd Futamura projections:
> *"The 3rd Futamura Projection ($cogen = \alpha(\alpha, \alpha)$) strictly requires specializing a self-applicable specializer with respect to itself... Running a supercompiler on a toy VM is NOT the 3rd projection."*

### 2.1 The Copy-Paste Trick in `minspec.nl`
Inspect lines 274–295 of `src/stdlib/minspec.nl`:
```numlang
// 1st Futamura Projection: prog_compiled = MinSpec(interp, prog)
fn first_futamura(prog_id: i64) -> SpecExpr {
    let interp: SpecExpr = make_interp_ast();
    let s_env: SpecEnv = SpecEnv::Cons(100, prog_id, box(SpecEnv::Nil));
    return min_spec(interp, s_env);
}

// 2nd Futamura Projection: compiler = MinSpec(MinSpec, interp)
fn second_futamura_compiler(prog_id: i64) -> SpecExpr {
    let interp: SpecExpr = make_interp_ast();
    let s_env: SpecEnv = SpecEnv::Cons(100, prog_id, box(SpecEnv::Nil));
    return min_spec(interp, s_env);
}

// 3rd Futamura Projection: cogen = MinSpec(MinSpec, MinSpec)
fn third_futamura_cogen(prog_id: i64) -> SpecExpr {
    let interp: SpecExpr = make_interp_ast();
    let s_env: SpecEnv = SpecEnv::Cons(100, prog_id, box(SpecEnv::Nil));
    return min_spec(interp, s_env);
}
```
**Analysis**:
1. All three functions have **identical function bodies**.
2. None of them invoke `min_spec(min_spec, ...)`.
3. In `verify_soundness` (lines 332–333), the assertion `expr_eq(p1, p2) && expr_eq(p2, p3)` succeeds solely because `p1`, `p2`, and `p3` were computed by calling the exact same function with the exact same arguments.
4. This directly violates `INTEGRITY_RULES.md` §5.

---

## 3. Systems & Runtime Flaws

### 3.1 Unbounded Memory Leaks (Zero Deallocation Runtime)
NumLang supports heap allocations via `box(expr)` and recursive types (`Box<T>`).
Inspect `src/codegen/cranelift_backend.rs` lines 1712–1745:
```rust
let size_val = builder.block_params(entry)[0];
let flags_val = builder.ins().iconst(types::I32, 0x0040); // LPTR = LMEM_FIXED | LMEM_ZEROINIT
let local_alloc = self.module.declare_func_in_func(local_alloc_id, builder.func);
let call = builder.ins().call(local_alloc, &[flags_val, size_val]);
let ptr = builder.inst_results(call)[0];
builder.ins().return_(&[ptr]);
```
**Analysis**:
- There is **no `free`**, **no `LocalFree`**, **no GC**, and **no destructor/RAII drop mechanism** anywhere in the runtime or codegen.
- Any long-running loop that allocates a closure, vector, or recursive list will steadily consume virtual memory until the process is killed by the OS.
- Calling NumLang a "systems programming language" while lacking any memory reclamation strategy is indefensible in production.

---

### 3.2 Hardcoded Windows APIs Break Linux & Docker Artifacts
The repository includes a `docker/Dockerfile` based on `rust:1.85-slim-bullseye` (Linux).
However, `src/codegen/cranelift_backend.rs` unconditionally declares and imports Win32 API functions:
```rust
// Lines 1495-1520 in cranelift_backend.rs:
let exit_process_id = module.declare_function("ExitProcess", Linkage::Import, &exit_sig)?;
let get_std_handle_id = module.declare_function("GetStdHandle", Linkage::Import, &gsh_sig)?;
let write_file_id = module.declare_function("WriteFile", Linkage::Import, &wf_sig)?;
```
And in `src/codegen/entry_bench.c` (line 33):
```c
ExitProcess((UINT)ret);
```
And in `emit_helper_print_str` (lines 1795–1802):
```rust
let std_out_handle = builder.ins().iconst(types::I32, -11); // STD_OUTPUT_HANDLE
let h_call = builder.ins().call(get_std_handle_func, &[std_out_handle]);
builder.ins().call(write_file_func, &[h_stdout, ptr, len, written_addr, zero64]);
```
**Analysis**:
- When compiling on Linux via `link_unix` (`cc obj.o -o exe -lm -no-pie`), the linker will fail with:
  ```
  undefined reference to `ExitProcess'
  undefined reference to `GetStdHandle'
  undefined reference to `WriteFile'
  ```
- The Docker reproduction pipeline in `docker/entrypoint.sh` will fail during binary linking unless running in Wine or an emulated Windows environment.

---

### 3.3 Numeric Overflow & Precision Loss in Recurrence Solvers
Inspect `src/mir/supercompiler/generalize.rs` (lines 440–451):
```rust
let disc = c1.wrapping_mul(c1).wrapping_add(4i64.wrapping_mul(c2));
if disc >= 0 {
    let d = (disc as f64).sqrt().round() as i64;
    if d * d == disc && (c1 + d) % 2 == 0 {
        let r1 = (c1 + d) / 2;
        let r2 = (c1 - d) / 2;
        let num_a = s1.wrapping_sub(s0.wrapping_mul(r2));
        let den_a = r1 - r2;
        let a = num_a / den_a;
        let b = s0 - a;
```
**Analysis**:
1. **Precision Loss**: `disc as f64` casts an `i64` to an IEEE 754 double. `f64` only has 53 bits of precision. For discriminants $> 2^{53} \approx 9 \times 10^{15}$, low bits are truncated, leading to false square detection or missed closed forms.
2. **Debug Panics**: `d * d`, `c1 + d`, `c1 - d`, `r1 - r2`, and `s0 - a` use **raw unchecked arithmetic**. When `d > 3 \times 10^9`, `d * d` will panic in debug mode due to integer overflow.
3. **Division by Zero**: If `den_a == 0` (which occurs if $r_1 = r_2$), line 450 will divide by zero and crash the compiler thread.

---

## 4. Supercompilation & SMT Translation Scaling Bottlenecks

### 4.1 Bounded SMT Translation Validation Ignores Loops
In `src/mir/supercompiler/validate.rs`, the certified translation validator claims to formally verify that the residual MIR is equivalent to the original MIR.
Inspect line 1834:
```rust
let orig_paths = orig_extractor.extract_paths(128, 64);
let res_paths = res_extractor.extract_paths(128, 64);
```
And inside `extract_paths` (lines 1434–1436):
```rust
if paths.len() >= max_paths || depth > max_depth {
    continue;
}
```
**Analysis**:
- Path extraction is **depth-bounded to 64** and **path-bounded to 128**.
- Any loop with a symbolic trip count or an iteration count $> 64$ is silently truncated via `continue`.
- This is bounded path checking, **not** certified unbounded translation validation. It cannot prove equivalence of loops with non-trivial loop invariants.

---

### 4.2 Compile-Time Explosions & Stack Crutches
- In `tests/benchmark_correctness_tests.rs`:
  - `test_canonical_double_nrev`: **28.29 seconds** for a ~30 line program.
  - `test_all_30_benchmarks_compile`: **27.29 seconds**.
  - `translation_validation_smt_tests`: **29.89 seconds**.
- In `src/main.rs` (lines 786–792):
  ```rust
  std::thread::Builder::new()
      .stack_size(16 * 1024 * 1024)
      .spawn(real_main)
  ```
  The compiler requires a **16 MB stack allocation** on startup to avoid blowing the default 1MB/2MB OS thread stack due to deep recursive tree unfolding.

---

## 5. Architectural Debt & Redundancy

| Component | Lines | File Size | Critical Flaw |
| :--- | :---: | :---: | :--- |
| `src/codegen/cranelift_backend.rs` | **9,306** | 444 KB | Massive monolith containing duplicate AST and MIR codegen pipelines with 200+ raw `.unwrap()` calls. |
| `src/typecheck/checker.rs` | **4,075** | 182 KB | Monolithic checker with nested ad-hoc pattern matching and hardcoded builtins. |
| `src/opt/supercompiler/` vs `src/mir/supercompiler/` | ~15,000 | ~600 KB | **Two completely parallel supercompilers** exist in the codebase: one at the AST level and one at the MIR level, causing CLI schizophrenia. |

---

## 6. Concrete Remediation Plan (Phases 41–45)

To convert NumLang from an academic paper draft with mock shortcuts into a genuine production-grade compiler, the following engineering phases are required:

### Phase 41: Decouple Win32 & Implement True POSIX Native Codegen
- Abstract libc and runtime system calls:
  - Windows: `ExitProcess`, `GetStdHandle`, `WriteFile`, `LocalAlloc`.
  - Linux/macOS: `exit`, `write(1, ...)`, `malloc`/`mmap`.
- Fix `entry_bench.c` to use standard C `exit(code)` instead of `ExitProcess`.
- Test Docker artifact natively on Linux.

### Phase 42: Replace Vacuous Lean 4 Tautologies with Constructive Proofs
- Connect `Step` to `Evaluates`: Define `Evaluates fn env res` as $\exists s_f, \text{Step}^* \langle 0, env \rangle s_f \land s_f.term = \text{Return}(\text{res})$.
- Implement actual transformations (dead node elimination, constant folding, interval pruning) as Lean definitions.
- Prove preservation: `Step* s_orig s_f \implies Step* (transform s_orig) (transform s_f)`.

### Phase 43: Implement Real Self-Applicable Specializer (`MinSpec`)
- Write an actual expression-level partial evaluator in `minspec.nl` that takes `(spec_program: SpecExpr, static_env: SpecEnv) -> SpecExpr`.
- Implement `second_futamura` as `min_spec(min_spec_ast, interp_ast)`.
- Implement `third_futamura` as `min_spec(min_spec_ast, min_spec_ast)`.

### Phase 44: Add Memory Management (Arena or Ref-Counting)
- Implement an explicit arena runtime (`__nl_arena_alloc`, `__nl_arena_reset`) for scoped batch allocations.
- Alternatively, implement an automatic reference counting (ARC) runtime pass for `Box<T>` and closures with compiler-emitted decrement instructions on scope exit.

### Phase 45: Monolith Decomposition & Codegen Unification
- Delete the legacy AST-level supercompiler (`src/opt/supercompiler/`) and enforce MIR as the sole optimization IR.
- Break `cranelift_backend.rs` into submodules (`abi.rs`, `builder.rs`, `intrinsics.rs`, `emit.rs`).
- Replace raw `.unwrap()` calls with structured `CodegenError` propagation.
