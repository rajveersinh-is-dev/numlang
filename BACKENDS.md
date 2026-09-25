# NumLang Compiler Backends: Cranelift vs. LLVM

NumLang features a dual-backend compilation model designed to maximize developer iteration speed while providing high-performance production binaries.

---

## 1. Architectural Summary

| Dimension | Cranelift (`--backend cranelift`) | LLVM (`--backend llvm`) |
|---|---|---|
| **Role** | Default, Development, Supercompiler JIT/AOT | Production Release Builds (`--release`) |
| **Dependencies** | Pure Rust (Zero external C/C++ dependencies) | Native LLVM 18+ toolchain |
| **Compilation Speed** | Blazing fast (< 15ms per file) | Moderate (runs whole-module optimization passes) |
| **Optimizer** | Fast SSA lowerer, peephole, linear-scan regalloc | Full `-O3`, vectorizer, GVN, mem2reg, loop unrolling |
| **Vectorization** | Manual SIMD / supercompiled intrinsics | Auto-vectorization (LoopVectorize + SLPVectorize) |
| **Target Architecture** | x86_64, AArch64, RiscV64 | Multi-architecture via LLVM targets |
| **Platform Linker** | Built-in MSVC / `rust-lld` driver | Standard COFF / ELF via platform linker |

---

## 2. When to Use Which Backend

### Use Cranelift When:
1. **Interactive Development**: Fast edit-compile-test cycles where compilation time matters most.
2. **Supercompiled Code**: Programs containing linear, polynomial, geometric, or coupled recurrences (such as Fibonacci, triangular sums, Newton's method). The NumLang supercompiler collapses these loops into $O(1)$ or $O(\log N)$ closed-form formulas before code generation, eliminating loop overhead entirely. Cranelift generates machine code for these closed forms in sub-milliseconds.
3. **Hermetic Environments**: Environments where installing large external C/C++ toolchains is impractical or undesirable.

### Use LLVM When:
1. **Production Release Builds**: Deployable binaries requiring peak CPU throughput.
2. **Dense Numerical Kernels**: Workloads not closed by the supercompiler (e.g., dense matrix multiplication $O(N^3)$, convolution, FFT, sorting networks, large floating-point arrays).
3. **Auto-Vectorization**: LLVM's `LoopVectorize` and `SLPVectorize` automatically emit AVX2 / AVX-512 vector instructions for scalar loops.
4. **Hardware Specific Tuning**: Target-specific instruction scheduling, microarchitectural tuning (`+avx2`, `+fma`, `+bmi2`), and aggressive loop unrolling (`-O3`).

---

## 3. CLI Usage

NumLang provides CLI options to select the backend and optimization level:

```bash
# Default build (Cranelift, fast compilation)
numlang build src/main.nl -o main.exe

# Release build using LLVM with -O3 optimizations
numlang build --backend llvm --opt-level 3 src/main.nl -o main.exe

# Run program directly with LLVM backend
numlang run --backend llvm --opt-level 3 src/main.nl

# Combine with supercompilation
numlang build --backend llvm --opt-level 3 --supercompile src/main.nl -o main.exe
```

### Optimization Levels (`--opt-level <0|1|2|3>`):
- `0`: No optimizations (debug mode, fastest codegen).
- `1`: Basic optimizations (`mem2reg`, instruction combining, CFG simplification).
- `2`: Standard optimizations (GVN, reassociation, basic vectorization) [Default for LLVM].
- `3`: Aggressive optimizations (full auto-vectorization, loop unrolling, SLP vectorization).

---

## 4. Benchmark Comparison

The comparative benchmark suite (`tests/llvm_vs_cranelift_benchmarks.rs`) measures compilation time and execution throughput across representative workloads:

| Workload | Type | Cranelift `-O0` | LLVM `-O3` | Speedup |
|---|---|---|---|---|
| **Fibonacci 1M** | Supercompiled Recurrence | $< 100\text{ ns}$ ($O(\log N)$) | $< 100\text{ ns}$ ($O(\log N)$) | $\approx 1.0\times$ |
| **Triangular Sum 50M** | Supercompiled Polynomial | $< 100\text{ ns}$ ($O(1)$) | $< 100\text{ ns}$ ($O(1)$) | $\approx 1.0\times$ |
| **Dense Matrix Mul $100 \times 100$** | Non-closed Loop Nest | $4.2\text{ ms}$ | $1.1\text{ ms}$ | $\mathbf{3.8\times}$ |
| **Sort / Scan (2K Elements)** | Dynamic Memory Access | $18\text{ ms}$ | $9\text{ ms}$ | $\mathbf{2.0\times}$ |

*Key Takeaway*:
- For programs closed by NumLang's supercompiler, Cranelift and LLVM achieve identical microsecond-level performance because the recurrence was eliminated mathematically before codegen.
- For non-closed iterative algorithms and array traversals, LLVM's auto-vectorization and loop unrolling provide substantial speedups ($2\times - 4\times$).

---

## 5. Cross-Compilation and Future Work

Future releases will extend the LLVM backend with:
- **Link-Time Optimization (LTO)**: ThinLTO and Full LTO across multi-module NumLang packages.
- **Polyhedral Loop Optimization**: Polly integration for automatic loop tiling and cache locality optimization.
- **Target Triples**: Cross-compilation support for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, and WebAssembly (`wasm32-unknown-unknown`).
