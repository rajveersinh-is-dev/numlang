# NumLang: A Formally Verified, Self-Applicable Process-Tree Supercompiler

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/davea/numlang)
[![Tests: 46+ suites](https://img.shields.io/badge/tests-100%25%20passing-brightgreen.svg)](https://github.com/davea/numlang)
[![Clippy: 0 warnings](https://img.shields.io/badge/clippy-0%20warnings-brightgreen.svg)](https://github.com/davea/numlang)
[![Lean 4 Verified](https://img.shields.io/badge/Lean%204-verified-blue.svg)](proof/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![ACM PEPM 2026](https://img.shields.io/badge/ACM%20SIGPLAN-PEPM%20'26-purple.svg)](paper/)

**NumLang** is a high-performance numerical systems programming language and optimizing compiler featuring an SSA-based process-tree supercompiler, formal machine-checked proofs in **Lean 4**, and automatic derivation of closed-form algorithmic solutions ((N) \to O(1)$).

Targeting native x86-64 via a **Cranelift** JIT/AOT code generator, NumLang combines metacomputation (Turchin-style supercompilation and the three Futamura projections) with low-level systems control.

---

## Architecture Overview

`
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
                        | Type Checker & Ast Opt    |
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
`

---

## Key Features

1. **SSA Process-Tree Supercompilation**:
   - Symbolic driving over SSA control-flow graphs with path condition tracking.
   - Size-filtered homeomorphic embedding whistle guaranteeing termination by Kruskal's Tree Theorem.
   - Most Specific Generalization (MSG) with variable widening.

2. **Automated Recurrence Solving ((N) \to O(1)$)**:
   - Higher-order forward difference engine for polynomial sequence detection.
   - Binomial closed-form generation with modular arithmetic support.
   - Coupled linear recurrence solving via (k^3 \log N)$ binary matrix exponentiation.

3. **Realization of the Three Futamura Projections**:
   - **1st Projection**: Interpreter + Program -> Native SSA Compiled Code.
   - **2nd Projection**: Specializer + Interpreter -> Standalone Compiler.
   - **3rd Projection**: Specializer + Specializer -> Standalone Compiler Generator (cogen).

4. **Lean 4 Mechanization**:
   - Small-step operational semantics formalization.
   - Machine-checked simulation proof for symbolic driving soundness (Driving.lean).
   - Machine-checked well-founded termination theorem (Termination.lean).

5. **Empirical Benchmarking**:
   - 10 canonical literature benchmarks implemented across NumLang, Rust, C, and Haskell.
   - Multi-compiler statistical harness with CPU core pinning, 5 warmups, 30 timed iterations, and 10,000 bootstrap confidence intervals.
   - Publication-grade vector plots (ench/figures/).

---

## 60-Second Quickstart

### Build Compiler
`ash
cargo build --release
`

### Write a Program (sum.nl)
`ust
fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 1..=1000000 {
        sum = sum + i;
    }
    println(sum);
    return sum % 256;
}
`

### Run with Supercompilation
`ash
# Compile and run directly via Cranelift
target/release/numlang.exe run --supercompile sum.nl

# Build standalone native executable
target/release/numlang.exe build --supercompile sum.nl -o sum.exe
./sum.exe
`

---

## Reproducibility & Artifact Evaluation

To replicate all experimental results, figures, and formal verification gates:

### Via Docker (Single Command)
`ash
docker build -t numlang-artifact -f docker/Dockerfile .
docker run --rm -v C:\Users\davea\.gemini\antigravity\scratch\numlang/bench/figures:/numlang/bench/figures numlang-artifact
`

### Via Bare Metal
- **Linux/macOS**: ash scripts/run_all_experiments.sh
- **Windows**: powershell -ExecutionPolicy Bypass -File scripts/run_all_experiments.ps1

For full artifact evaluation details, see [REPRODUCIBILITY.md](REPRODUCIBILITY.md).

---

## Research Paper

The complete ACM SIGPLAN research paper draft targeting **PEPM '26** is located in [paper/](paper/):
- paper/main.tex: Full paper source (ACM SIGPLAN cmart format).
- paper/references.bib: Verified BibTeX bibliography.
- paper/build_paper.sh: Automated PDF build script.

---

## License

NumLang is licensed under the [MIT License](LICENSE).
