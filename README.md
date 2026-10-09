# NumLang

> An experimental research programming language integrating SSA supercompilation, polyhedral recurrence solving, and Lean 4 mechanized operational models.

[![CI](https://github.com/rajveersinh-is-dev/numlang/actions/workflows/ci.yml/badge.svg)](https://github.com/rajveersinh-is-dev/numlang/actions/workflows/ci.yml)
[![Lean 4 Proofs](https://github.com/rajveersinh-is-dev/numlang/actions/workflows/lean.yml/badge.svg)](https://github.com/rajveersinh-is-dev/numlang/actions/workflows/lean.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

---

## 1. What is NumLang?

**NumLang** is an experimental compiled research language and program optimizer implemented in Rust. It integrates multiple program transformation paradigms into a unified SSA Mid-Level Intermediate Representation (MIR) pipeline:

- **Hamilton-style Global Distillation**: Inter-procedural process tree distillation that folds across distinct call sites to eliminate intermediate algebraic structures (e.g. multi-stage stream and tree traversals).
- **Multi-Result Supercompilation (MRSC)**: Bounded configuration hypergraph search paired with Pareto-optimal candidate selection under register and instruction cost models.
- **Polyhedral Recurrence Detection**: Algebraic difference engine detecting constant forward differences and companion matrices, collapsing arithmetic progressions and linear recurrences.
- **Reynolds Defunctionalization**: Type-directed whole-program closure conversion mapping higher-order lambdas into first-order tagged variants with static dispatches.
- **Lazy Thunk / Codata Supercompilation**: Demand-driven symbolic forcing over SSA MIR converting lazy producer-consumer stream pipelines into scalar register loops.
- **Self-Applicable Specialization**: A subset specializer (`src/stdlib/minspec.nl`) structured for Futamura projections (specializing interpreters into compiled residuals).
- **Lean 4 Mechanized Operational Models**: Formal machine-checked small-step operational semantics and semantic preservation proofs with **zero `sorry`** and **zero unproven axioms** (`lean/`).

---

## 2. Compiler Pipeline

```mermaid
flowchart LR
    A["Source (.nl)"] --> B[Logos Lexer]
    B --> C[Pratt Parser]
    C --> D[Typed AST]
    D --> E[Typechecker]
    E --> F[IR Lowering]
    F --> G[SSA MIR]
    G --> H{Supercompiler}
    H -->|--supercompile| I[Residualized MIR]
    H -->|baseline| I
    I --> J[Cranelift Backend]
    I --> K[LLVM Backend]
    J --> L[Native Executable]
    K --> L
```

Programs are parsed into a typed AST and lowered to an SSA MIR with explicit memory tokens (`MemorySSA`), field-sensitive alias analysis, and `Mem2Reg`. When `--supercompile` is enabled, the supercompiler executes symbolic driving, most-specific generalization (MSG), knot-tying, and recurrence analysis before emitting optimized residual MIR to Cranelift or LLVM.

---

## 3. Quick Start

### Prerequisites
- **Rust**: Stable toolchain ($\ge 1.82$) with `rustfmt` and `clippy`.
- **Optional**: LLVM 18+ for `--backend llvm`, Lean 4 (`elan`) for proof verification, Python 3.10+ for benchmarks.

### Building & Running

```bash
# Clone the repository
git clone https://github.com/rajveersinh-is-dev/numlang.git
cd numlang

# Build release binary
cargo build --release

# Run tests
cargo test

# Compile and run a sample NumLang program
cargo run --bin numlang -- run examples/fib.nl

# Compile with full supercompiler optimizations
cargo run --bin numlang -- run --supercompile examples/sum_of_squares.nl
```

*(On Windows PowerShell, use `cargo run --bin numlang -- run examples\fib.nl`)*

---

## 4. Claims and Evidence

Every optimization capability in NumLang is verifiable with specific automated commands and regression tests:

| Architectural Claim | Verifying Test / Command | Expected Output | Status |
|:---|:---|:---|:---:|
| **Zero Production Invariant Shortcuts** | `cargo clippy --all-targets -- -D warnings` | 0 errors, 0 warnings across all targets | Verified |
| **Algorithmic Generality (No Name Matching)** | `cargo test --test integrity_lint`<br>`cargo test --test structural_generality_tests` | 7 passed, 0 failed; no string matches on benchmark identifiers | Verified |
| **Arbitrary Order-2 Linear Recurrences** | `cargo test --test general_recurrence_tests` | 3 passed; Fib, Lucas, Pell, Jacobsthal symbolic & execution | Verified |
| **Differential Correctness vs Oracle** | `cargo test --test differential_correctness_tests` | 2 passed; 0 divergences across generated programs | Verified |
| **Machine-Checked Lean 4 Proofs** | `cd lean && lake build` | Clean build, 0 `sorry`, 0 unproven `axiom` | Verified |
| **Monograph Compilation & Cross-References** | `python paper/book/audit_pdf.py` | 374 pages, 0 broken cross-refs (`??`), 0 broken cites (`[?]`) | Verified |
| **Multi-Stage Futamura Specialization** | `cargo test --test third_futamura_tests` | All projections produce executable residual binaries | Verified |

---

## 5. Related Work and Prior Art

NumLang builds on decades of foundational research in metacomputation, supercompilation, and formal compiler verification:

1. **Higher-Order Supercompilation (HOSC)**: Ilya Klyuchnikov (2010) demonstrated higher-order positive supercompilation for functional programs with homeomorphic embedding whistles.
2. **Supero**: Neil Mitchell (2008) explored practical supercompilation for core Haskell, focusing on let-inlining and operational termination.
3. **Multi-Result Supercompilation (MRSC)**: Ilya Klyuchnikov and Sergei Romanenko (2011) formulated configuration generator hypergraphs and generalized whistle frameworks.
4. **Program Distillation**: Geoff Hamilton (2007) introduced global distillation to eliminate intermediate data structures across distinct call trees.
5. **LLVM Scalar Evolution (SCEV) & Polyhedral Compilers**: Modern systems compilers (e.g. Clang/LLVM, GCC) use algebraic recurrence analysis (SCEV) and polyhedral loop engines (Polly, Pluto) to collapse and vectorize loops.
6. **Mechanized Metacomputation**: Verified supercompiler and partial evaluation models in Coq and Lean (e.g. SPSC mechanization, Jones & Gomard foundations).

---

## 6. Empirical Benchmark Results

All timings are collected using in-process hardware performance counters (`QueryPerformanceCounter` on Windows, `clock_gettime` on Linux) over **at least 5 discarded warmups followed by 30 measured rounds per cell**:

| Benchmark | Category | NumLang-SC | NumLang-Base | Rustc -O | MSVC /O2 | Note |
|:---|:---|:---:|:---:|:---:|:---:|:---|
| Naive Reverse (`nrev`) | Deforestation | 264.9 µs | 156.0 µs | 1.76 ms | 53.6 µs | List allocation dominates |
| Triple Append (`append3`) | Deforestation | 104.7 µs | 68.4 µs | 447.2 µs | 14.9 µs | List allocation dominates |
| Knuth-Morris-Pratt (KMP) | Specialization | 40.7 µs | 31.0 µs | 274.1 µs | 61.5 µs | Specialized DFA matching |
| Peano Multiplication | Specialization | 38.0 µs | 33.0 µs | 129.9 µs | 185.2 µs | Unfolded Peano arithmetic |
| Double Tree Inversion | Deforestation | 112.1 µs | 67.3 µs | 531.0 µs | 509.4 µs | Intermediate tree pruned |
| Fibonacci Matrix Power | Recurrences | 14.6 µs | 100.4 µs | 3.5 µs | 34.8 µs | $O(N) \to O(\log N)$ exponentiation |
| Triangular Summation (50M) | Recurrences | ~100 ns | 12.57 ms | ~0 ns | 9.46 ms | Collapsed algebraically to $O(1)$ |
| Cubic Polynomial Sum (10M) | Recurrences | ~100 ns | 4.00 ms | ~0 ns | 3.58 ms | Collapsed algebraically to $O(1)$ |
| Sum-Map Stream Fusion (1M) | Stream Fusion | ~100 ns | 393.3 µs | 984.1 µs | 355.5 µs | Fused single-pass loop |
| Filter-Sum Stream Pipeline | Stream Fusion | 11.7 µs | 45.3 µs | 26.5 µs | 37.0 µs | Deforested intermediate buffer |

> **Honest Comparison Note**: Both NumLang-SC and LLVM-based compilers (such as `rustc -O`) collapse constant-bound arithmetic loops like Triangular Summation down to instantaneous $O(1)$ scalar answers at compile time using scalar evolution (SCEV). NumLang performs this transformation at the SSA process tree level via difference recurrence solving, independent of backend codegen.

---

## 7. Honest Limitations

NumLang is engineered under strict computational honesty:

1. **Linked Data Structure Overheads**: For pointer-linked lists and trees (`nrev`, `append3`), memory allocation overheads in the Cranelift backend currently exceed the benefit of process-tree folding compared to heavily tuned C runtime allocators.
2. **Backend Vectorization Gap**: For non-collapsed numerical kernels, LLVM `-O3` generates AVX2/AVX-512 SIMD vector loops that outperform Cranelift baseline emission.
3. **Lean 4 Model Boundary**: The Lean 4 formalization verifies an abstract operational model of the core language and transformations. The Rust compiler binary is not mechanically extracted from Lean.

---

## 8. Integrity Rules

Development in this repository strictly enforces [`INTEGRITY_RULES.md`](INTEGRITY_RULES.md):
- **No Preloaded Numbers / Constant Lookups**: Zero synthetic benchmark shortcutting or precomputed recurrence tables.
- **Computational Honesty**: In-process microsecond hardware performance counters with statistical replication.
- **Purity & Type Safety**: Zero `panic!()` in code generators, zero `.unwrap()` in lowering pipelines, and clean `cargo clippy --all-targets -- -D warnings`.
- **Algorithmic Generality**: Compilers and optimizers must treat all user code symmetrically without matching function or variable names.

---

## 9. Citation

```bibtex
@misc{numlang2026,
  title  = {NumLang: A Research Compiler with SSA Supercompilation and Mechanized Correctness},
  author = {Rajveersinh Pardeshi},
  year   = {2026},
  url    = {https://github.com/rajveersinh-is-dev/numlang}
}
```

---

## 10. License

Distributed under the MIT License. See [`LICENSE`](LICENSE) for details.
