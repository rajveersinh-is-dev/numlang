# NumLang: A Formally Verified, Self-Applicable Process-Tree Supercompiler

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/Raj123-0/numlang)
[![Tests: 102 suites](https://img.shields.io/badge/tests-100%25%20passing-brightgreen.svg)](https://github.com/Raj123-0/numlang)
[![Clippy: 0 warnings](https://img.shields.io/badge/clippy-0%20warnings-brightgreen.svg)](https://github.com/Raj123-0/numlang)
[![Lean 4 Verified](https://img.shields.io/badge/Lean%204-verified-blue.svg)](lean/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![ACM PLDI 2027](https://img.shields.io/badge/ACM%20SIGPLAN-PLDI%20'27-purple.svg)](paper/)

**NumLang** is a high-performance numerical systems programming language and optimizing compiler featuring an SSA-based process-tree supercompiler, formal machine-checked proofs in **Lean 4**, and automatic derivation of closed-form algorithmic solutions ($O(N) \to O(1)$).

Targeting native x86-64 via **Cranelift** and **LLVM** code generators, NumLang combines metacomputation (Turchin-style supercompilation and the three Futamura projections) with low-level systems control.

---

## Architecture Overview

```
                        +---------------------------+
                        |  NumLang Source (.nl)     |
                        +-------------+-------------+
                                      |
                                      v
                        +---------------------------+
                        |  Lexer & Pratt Parser     |
                        +-------------+-------------+
                                      |
                                      v
                        +---------------------------+
                        | Type Checker & AST Opt    |
                        +-------------+-------------+
                                      |
                                      v
                        +---------------------------+
                        | SSA Mid-Level IR (MIR)    |
                        +-------------+-------------+
                                      |
                 +--------------------+--------------------+
                 |                                         |
                 v                                         v
   +---------------------------+             +---------------------------+
   |  SSA Process-Tree Driver  |             |  Stream Fusion Pass       |
   |  - Symbolic Driving       |             |  - Deforestation          |
   |  - Path Constraints       |             |  - Buffer Elimination     |
   |  - Size-Filtered Whistle  |             +-------------+-------------+
   +-------------+-------------+                           |
                 |                                         |
                 v                                         |
   +---------------------------+                           |
   | Recurrence Closed-Form    |                           |
   | - Polynomial Diff Engine  |                           |
   | - Matrix Exponentiation   |                           |
   +-------------+-------------+                           |
                 |                                         |
                 +--------------------+--------------------+
                                      |
                                      v
                        +---------------------------+
                        |  Residualized SSA CFG     |
                        +-------------+-------------+
                                      |
                                      v
                        +---------------------------+
                        |  Cranelift Native Codegen |
                        +-------------+-------------+
                                      |
                                      v
                        +---------------------------+
                        | Standalone Binary (.exe)  |
                        +---------------------------+
```

---

## Key Features

1. **SSA Process-Tree Supercompilation**:
   - Symbolic driving over SSA control-flow graphs with path condition tracking.
   - Size-filtered homeomorphic embedding whistle guaranteeing termination by Kruskal's Tree Theorem.
   - Most Specific Generalization (MSG) with variable widening.

2. **Automated Recurrence Solving ($O(N) \to O(1)$)**:
   - Higher-order forward difference engine for polynomial sequence detection.
   - Binomial closed-form generation with modular arithmetic support.
   - Coupled linear recurrence solving via $O(k^3 \log N)$ binary matrix exponentiation.

3. **Realization of the Three Futamura Projections**:
   - **1st Projection**: Interpreter + Program $\to$ Native SSA Compiled Code.
   - **2nd Projection**: Specializer + Interpreter $\to$ Standalone Compiler.
   - **3rd Projection**: Specializer + Specializer $\to$ Standalone Compiler Generator (cogen).

4. **Lean 4 Mechanization**:
   - Small-step and big-step operational semantics formalization.
   - Machine-checked simulation proof for symbolic driving soundness (`Semantics.lean`, `Distillation.lean`, `MRSC.lean`).
   - Machine-checked well-founded termination theorem (`Termination.lean`).
   - End-to-end soundness theorem (`Main.lean`) verified with **zero unproven axioms** and **zero `sorry`**.

5. **Empirical Benchmarking**:
   - 30 diverse benchmarks implemented across NumLang, Rust, C, and Haskell.
   - Multi-compiler statistical harness with CPU core pinning, 5 warmups, 30 timed iterations, and 10,000 bootstrap confidence intervals.
   - Publication-grade vector plots (`bench/figures/`).

---

## 60-Second Quickstart

### Build Compiler
```bash
cargo build --release
```

### Write a Program (`sum.nl`)
```rust
fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 1..=1000000 {
        sum = sum + i;
    }
    println(sum);
    return sum % 256;
}
```

### Run with Supercompilation
```bash
# Compile and run directly via Cranelift
target/release/numlang run --supercompile sum.nl

# Build standalone native executable
target/release/numlang build --supercompile sum.nl -o sum.exe
./sum.exe
```

---

## Reproducibility & Artifact Evaluation

To replicate all experimental results, figures, formal verification gates, and paper compilation:

### 1-Click Reproducible Docker Artifact
```bash
make artifact-docker
# or directly:
docker build -f docker/Dockerfile -t numlang-artifact .
docker run --rm numlang-artifact
```

### Via Local Makefile
```bash
make build       # Build compiler in release mode
make test        # Run all test suites
make reproduce   # Run 30 benchmarks, regenerate all tables, verify SHA-256 checksums
make paper       # Compile paper/main.tex into publication-ready PDF
```

### Verified Artifact Structure
- `paper/main.tex`: Full ACM PLDI 2027 format research paper with modular sections in `paper/sections/`.
- `paper/references.bib`: Verified BibTeX entries.
- `bench/data/checksums.sha256`: Cryptographic checksums of all empirical tables and data.
- `rebuttal/likely_objections.md`: Pre-written, data-backed rebuttals to the 5 most anticipated reviewer objections.
- `lean/`: Machine-checked Lean 4 formalization with zero unproven axioms and zero `sorry`.

---

## Research Paper

The complete research paper targeting **ACM SIGPLAN PLDI 2027 / ICFP 2027** is located in [`paper/`](paper/):
- `paper/main.tex`: Top-level paper assembly importing `paper/sections/01_introduction.tex` through `09_conclusion.tex`.
- `paper/figures/`: Head-to-head performance (`table_head_to_head.tex`), ablation study (`table_ablation.tex`), code size (`table_codesize.tex`), and termination witness (`table_termination.tex`).
- `rebuttal/likely_objections.md`: Addressing benchmark diversity, proof scope, parallel scaling, GHC comparison, and specialization cache.

---

## License

NumLang is licensed under the [MIT License](LICENSE).
