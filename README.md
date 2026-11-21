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
- **Lean 4 Mechanized Operational Models**: Formal machine-checked small-step operational semantics and semantic preservation proofs with **zero `sorry`** and **zero unproven axioms** (see [`docs/LEAN_STATUS.md`](docs/LEAN_STATUS.md)).

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

Every optimization capability in NumLang is verifiable with specific automated commands and regression tests. All public claims are tracked in the [`docs/CLAIMS_LEDGER.md`](docs/CLAIMS_LEDGER.md):

| Architectural Claim | Verifying Test / Command | Expected Output | Status |
|:---|:---|:---|:---:|
| **Zero Production Invariant Shortcuts** | `cargo clippy --all-targets -- -D warnings` | 0 errors, 0 warnings across all targets | Verified |
| **Algorithmic Generality (No Name Matching)** | `cargo test --test integrity_lint`<br>`cargo test --test structural_generality_tests` | 7 passed, 0 failed; no string matches on benchmark identifiers | Verified |
| **Arbitrary Order-2 Linear Recurrences** | `cargo test --test general_recurrence_tests` | 3 passed; Fib, Lucas, Pell, Jacobsthal symbolic & execution | Verified |
| **Differential Correctness vs Oracle** | `cargo test --test differential_correctness_tests` | 3 passed; 0 divergences across examples and test suites | Verified |
| **Machine-Checked Lean 4 Proofs** | `cd lean && lake build`<br>`cd proof && lake build` | Clean build, 0 `sorry`, 0 unproven `axiom` (see [`docs/LEAN_STATUS.md`](docs/LEAN_STATUS.md)) | Verified |
| **Monograph Compilation & Cross-References** | `python paper/book/audit_pdf.py` | 374 pages, 0 broken cross-refs (`??`), 0 broken cites (`[?]`) | Verified |
| **Multi-Stage Futamura Specialization** | `cargo test --test third_futamura_tests` | All projections produce executable residual binaries | Verified |

---

## 5. Related Work and Prior Art

NumLang builds on decades of foundational research in metacomputation, supercompilation, and formal compiler verification:

1. **The Concept of a Supercompiler**: Valentin F. Turchin (1986, ACM TOPLAS 8(3):292–325) introduced supercompilation as generalized program execution over parameterized input domains.
2. **Deforestation**: Philip Wadler (1990, Theoretical Computer Science 73(2):231–248) established the transformation principles for eliminating intermediate data structures without creating trees.
3. **Program Distillation**: Geoff W. Hamilton (2007, ACM PEPM '07, pp. 61–70) formulated global distillation to eliminate intermediate data structures across distinct function call trees that supercompilation cannot fuse.
4. **Multi-Result Supercompilation (MRSC) & Generalization**: Robert Glück and Morten Heine Sørensen (1995), Andrei V. Klimov (2008), and Ilya G. Klyuchnikov & Sergei Romanenko (2012) formulated configuration generator hypergraphs and Pareto-optimal candidate selection over multi-result transformation spaces.
5. **Well-Quasi-Orderings & Whistle Termination**: Graham Higman (1952, Proc. London Math. Soc. s3-2(1):326–336) and Joseph B. Kruskal (1960, Trans. AMS 95(2):210–225) proved well-quasi-ordering properties underpinning homeomorphic embedding whistles (`whistle.rs`) to ensure supercompilation termination.
6. **Higher-Order Supercompilation & Practical Systems**: Ilya G. Klyuchnikov (2010, HOSC/SPSC) and Neil Mitchell (2008, 2010, ICFP '10) demonstrated practical supercompilation for functional languages.
7. **Partial Evaluation Projections**: Yoshihiko Futamura (1971, Systems, Computers, Controls 2(5):45–50) established the three Futamura projections for deriving residual compilers and compiler-compilers via partial evaluation.
8. **LLVM Scalar Evolution (SCEV) & Polyhedral Compilers**: Industrial systems compilers (Clang/LLVM, GCC) use algebraic recurrence analysis (SCEV) and polyhedral loop engines (Polly, Pluto) to collapse and vectorize loops.
9. **Mechanized Metacomputation**: Verified supercompiler and partial evaluation models in Coq and Lean 4 (Jones & Gomard 1993, de Moura & Ullrich 2021).

---

## 6. Empirical Benchmark Results

Full empirical benchmark evaluation against industrial compilers is documented in [`SHOWDOWN.md`](SHOWDOWN.md). All timings are collected using in-process hardware performance counters (`QueryPerformanceCounter` on Windows, `clock_gettime` on Linux) over **5 discarded warmups followed by 30 measured rounds per cell**. Wins vs baseline are strictly defined as: 95% CI of the ratio excludes 1.0 AND effect $\ge 5\%`:

| Benchmark | Category | NumLang-SC | NumLang-Base | Rustc-O3 | MSVC-O2 | Clang-O3 | GCC-O3 | GHC-O2 | Result vs Base (95% CI) |
|:---|:---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Naive Reverse (Double nrev) (`nrev`) | G1: Deforestation | 42.60 µs | 286.20 µs | 1.31 ms | 53.70 µs | 49.80 µs | 26.90 µs | 25.50 µs | **NumLang-SC Win (6.57x [4.77-8.90])** |
| Triple List Append (`append3`) | G1: Deforestation | 93.00 µs | 109.80 µs | 446.10 µs | 15.40 µs | 14.00 µs | 8.70 µs | 11.00 µs | **Tie (1.19x [0.74-1.84])** |
| Knuth-Morris-Pratt DFA (`kmp`) | G1: Deforestation | 55.20 µs | 51.40 µs | 225.90 µs | 60.40 µs | 62.90 µs | 210.20 µs | 10.50 µs | **Tie (1.05x [0.59-1.58])** |
| Peano Multiplication (`peano_mul`) | G1: Deforestation | 42.00 µs | 39.80 µs | 134.20 µs | 193.10 µs | 193.70 µs | 204.70 µs | 9.90 µs | **Tie (0.93x [0.62-1.17])** |
| Double Tree Inversion (`tree_flip`) | G1: Deforestation | 87.80 µs | 113.70 µs | 541.70 µs | 509.10 µs | 512.90 µs | 642.00 µs | 15.70 µs | **Tie (1.34x [1.01-2.02])** |
| Coupled Fibonacci Recurrence Matrix Power (`fib_matrix`) | G2: Recurrences | 11.70 µs | 126.80 µs | 3.40 µs | 34.80 µs | 3.10 µs | 2.50 µs | 6.80 µs | **NumLang-SC Win (10.79x [4.40-13.09])** |
| Triangular Summation (50M) (`tri_sum`) | G2: Recurrences | 12.60 µs | 21.69 ms | ≤ 500 ns* | 9.92 ms | ≤ 500 ns* | ≤ 500 ns* | 16.58 ms | **NumLang-SC Win (1688.80x [1129.13-2383.81])** |
| Sum of Squares 1^2+...+10M^2 (Degree-3) (`cubic_sum`) | G2: Recurrences | 13.30 µs | 5.03 ms | ≤ 500 ns* | 3.56 ms | ≤ 500 ns* | 1.42 ms | 4.54 ms | **NumLang-SC Win (379.58x [231.53-466.54])** |
| Geometric Power Loop (100) (`pow2_mod`) | G2: Recurrences | 13.20 µs | 11.90 µs | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | WRONG_OUTPUT(actual=1;expected=0) | **Tie (0.92x [0.47-1.45])** |
| Hofstadter Mutual Linear Recurrence (`hofstadter`) | G2: Recurrences | 13.00 µs | 14.70 µs | 6.50 µs | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | 22.70 µs | **Tie (1.14x [0.60-1.78])** |
| Dynamic Triangular Summation (50M) (`tri_sum_dyn`) | G2-Dyn: Recurrences (Runtime) | 14.30 µs | 21.05 ms | 6.26 ms | 12.68 ms | ≤ 500 ns* | 5.06 ms | 16.50 ms | **NumLang-SC Win (1489.28x [989.96-1920.63])** |
| Dynamic Sum of Squares (10M) (`cubic_sum_dyn`) | G2-Dyn: Recurrences (Runtime) | 14.70 µs | 4.54 ms | 4.12 ms | 3.57 ms | ≤ 500 ns* | 1.56 ms | 4.46 ms | **NumLang-SC Win (309.05x [237.79-509.43])** |
| Dynamic Fibonacci Matrix Power (1000) (`fib_matrix_dyn`) | G2-Dyn: Recurrences (Runtime) | 20.30 µs | 16.30 µs | 4.50 µs | 3.40 µs | 3.10 µs | 3.10 µs | 110.50 µs | **Tie (0.85x [0.37-1.30])** |
| Dynamic Geometric Power Loop (100) (`pow2_mod_dyn`) | G2-Dyn: Recurrences (Runtime) | 14.20 µs | 13.20 µs | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | WRONG_OUTPUT(actual=1;expected=0) | **Tie (1.01x [0.54-1.37])** |
| 5-Deep Function Composition Chain (`compose5`) | G3: Higher-Order | 13.30 µs | 13.50 µs | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | 0.60 µs | **Tie (1.15x [0.71-1.91])** |
| Map-Map Pipeline Deforestation (`map_map`) | G3: Higher-Order | 17.30 µs | 18.60 µs | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | ≤ 500 ns* | 7.20 µs | **Tie (1.05x [0.59-1.85])** |
| Sum-Map Stream Fusion (1M) (`sum_map`) | G3: Higher-Order | 12.40 µs | 434.90 µs | ≤ 500 ns* | 355.40 µs | ≤ 500 ns* | ≤ 500 ns* | 410.20 µs | **NumLang-SC Win (37.64x [16.47-49.15])** |
| Stream Pipeline Filter-Sum (`stream_take`) | G3: Higher-Order | 63.60 µs | 66.40 µs | ≤ 500 ns* | 37.10 µs | ≤ 500 ns* | 22.70 µs | WRONG_OUTPUT(actual=1;expected=0) | **Tie (0.98x [0.73-1.52])** |
| Bytecode Expression Evaluator (`expr_eval`) | G4: Extended Realistic | 11.00 µs | 11.50 µs | 39.10 µs | 6.80 µs | 9.40 µs | 9.90 µs | NOT_INSTALLED | **Tie (1.02x [0.68-1.95])** |
| Sliding-Window Sensor Statistics (`window_stats`) | G4: Extended Realistic | 11.70 µs | 12.80 µs | 34.00 µs | 9.50 µs | 13.00 µs | 9.40 µs | NOT_INSTALLED | **Tie (1.06x [0.45-1.49])** |
| Struct Record Transform Pipeline (`record_pipeline`) | G4: Extended Realistic | 13.80 µs | 15.60 µs | 32.90 µs | 7.70 µs | 10.30 µs | 9.60 µs | NOT_INSTALLED | **Tie (1.24x [0.72-5.18])** |
| Multi-Pass Clamped 2D Stencil (`nested_stencil`) | G4: Extended Realistic | 16.70 µs | 17.20 µs | 36.60 µs | 11.60 µs | 11.40 µs | 11.00 µs | NOT_INSTALLED | **Tie (1.04x [0.74-2.14])** |

> **Honest Comparison Note**: Both NumLang-SC and LLVM-based compilers (`rustc -C opt-level=3`) collapse constant-bound arithmetic loops like Triangular Summation down to instantaneous $O(1)$ scalar answers at compile time using scalar evolution (SCEV). Entries marked `≤ 500 ns*` evaluate within the hardware performance counter quantization floor ($\le 500$ ns) and are classified transparently as folded/invalid rather than claimed as numeric wins.

---

## 7. Honest Limitations

NumLang is engineered under strict computational honesty:

1. **Linked Data Structure Overheads & Allocator Attribution**: On pointer-linked lists and trees (`nrev`, `append3`, `tree_flip`), supercompilation eliminates intermediate constructor pairs but cannot eliminate the final list traversal or construction without algebraic identity simplification (`nrev(nrev(xs)) == xs`). Because NumLang-SC and NumLang-Base share the identical Cranelift runtime allocator backend, their execution times are within statistical noise of each other. Furthermore, our allocator attribution experiments (`docs/evidence/round2/allocator_attribution.csv`) confirm that the 25x gap between Rustc-O3 and MSVC-O2 on `nrev` was an allocator artifact (Windows `HeapAlloc` vs static bump pool), disappearing when Rust uses an equivalent bump arena (57.3 µs vs 53.6 µs).
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
