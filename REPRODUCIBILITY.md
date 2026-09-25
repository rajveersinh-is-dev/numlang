# Artifact Evaluation & Reproducibility Guide: NumLang

This guide provides step-by-step instructions to reproduce all empirical claims, formal verification proofs, and performance measurements presented in the paper:
> **"NumLang: A Formally Verified, Self-Applicable Process-Tree Supercompiler for Systems Languages"** (ACM SIGPLAN PEPM / POPL).

---

## 1. System Requirements

### Hardware Requirements
- **CPU**: x86-64 processor (Intel Core i5/i7/i9 or AMD Ryzen 5/7/9, 4+ physical cores recommended).
- **RAM**: Minimum 8 GB (16 GB recommended for running the full Criterion benchmark suite).
- **Disk Space**: ~8 GB free space (including Docker base images and toolchains).

### Software Requirements
- **Option A (Containerized — Recommended)**:
  - Docker Desktop or Docker Engine (20.10+) with Compose support.
- **Option B (Bare Metal)**:
  - **OS**: Linux (Debian 12, Ubuntu 22.04+), macOS 13+, or Windows 10/11 (MSVC 2019/2022).
  - **Rust**: `rustc` and `cargo` 1.80+ (tested on 1.81.0 and 1.98.1).
  - **C/C++**: `clang-17` / `gcc-12+` (or MSVC `cl.exe` on Windows).
  - **Haskell**: `ghc 9.4+` and `cabal 3.10+`.
  - **Proof Assistant**: `lean 4` (`v4.11.0`) with `elan` and `lake`.
  - **Python**: Python 3.10+ with `numpy`, `scipy`, `matplotlib`, and `pandas`.

---

## 2. Quickstart: 1-Click Docker Reproduction

To run the complete automated evaluation pipeline in a hermetic container:

```bash
# 1. Clone repository
git clone https://github.com/davea/numlang.git
cd numlang

# 2. Build the Docker image (installs Rust 1.81, LLVM 17, GHC, Lean 4, Python)
docker build -t numlang-artifact -f docker/Dockerfile .

# 3. Execute all verification gates, benchmarks, and figure generation
docker run --rm -v $(pwd)/bench/figures:/numlang/bench/figures numlang-artifact
```

This single command:
1. Executes all 46+ compiler unit and integration test suites (`cargo test --tests --release`).
2. Type-checks and verifies all formal machine proofs in Lean 4 (`lake build`).
3. Executes the multi-compiler empirical harness across all 10 canonical literature benchmarks (5 warmups + 30 timed iterations pinned to CPU 0).
4. Generates publication-ready vector figures (`bench/figures/speedup.pdf`, `throughput.pdf`, `codesize.pdf`).
5. Prints the complete empirical summary table comparing NumLang, Rustc, and Clang.

---

## 3. Bare Metal Reproduction

### Step 1: Build the Compiler
```bash
cargo build --release --all-features
```

### Step 2: Run the Comprehensive Test Suite
```bash
cargo test --tests --release
```
**Expected Outcome**: 100% test pass rate across all modules:
- Lexer, Pratt parser, and semantic type checker
- SROA, Mem2Reg SSA, BCE, inlining, and DSE
- Process-tree driving, whistle, and recurrence generalization
- Futamura projection interpreter specialization (1st, 2nd, and 3rd projections)
- Cranelift native code generation and Windows PE/COFF linking

### Step 3: Verify Lean 4 Formal Proofs
```bash
cd proof
lake build
cd ..
```
**Expected Outcome**: Zero proof errors or unfilled `sorry` axioms. Verifies:
- `Semantics.lean`: Small-step operational semantics and value preservation.
- `Driving.lean`: Full simulation and semantic soundness of symbolic driving.
- `Termination.lean`: Well-founded termination via Kruskal's Tree Theorem and homeomorphic embedding.

### Step 4: Execute the 10 Literature Benchmarks
```bash
# Run statistical benchmark runner (pinned to CPU 0, 5 warmups, 30 timed rounds)
python bench/harness/runner.py

# Regenerate publication vector figures
python bench/plot.py
```
**Expected Outcome**:
- Detailed statistical data written to `bench/data/results.csv`.
- High-resolution publication plots generated in `bench/figures/`:
  - `speedup.pdf`: Runtime performance comparison against baseline, Rust `-O3`, and C `/O2`.
  - `throughput.pdf`: Compilation and supercompilation throughput (lines/second).
  - `codesize.pdf`: Output executable binary footprint (bytes).

---

## 4. Mapping Paper Claims to Artifact Evidence

| Paper Claim | Artifact Component / Test Case | Primary Metric / Proof Target |
|:---|:---|:---|
| **Claim 1: Soundness of Symbolic Driving** | `proof/NumLangProofs/Driving.lean`<br>`tests/mir_tests.rs` | Theorem `driving_step_soundness`: Symbolic driving transitions simulate concrete operational steps. |
| **Claim 2: Guaranteed Supercompiler Termination** | `proof/NumLangProofs/Termination.lean`<br>`src/mir/supercompiler/whistle.rs` | Kruskal Tree Theorem: Homeomorphic embedding whistle ensures finite process trees. |
| **Claim 3: $O(N) \to O(1)$ Recurrence Derivation** | `tests/coupled_recurrence_and_fusion_tests.rs`<br>`src/opt/supercompiler/generalization.rs` | Polynomial sequences and coupled linear recurrences are collapsed to analytic closed forms. |
| **Claim 4: Deforestation & Stream Fusion** | `tests/deforestation_tests.rs`<br>`src/mir/supercompiler/fusion.rs` | Intermediate allocations eliminated; multi-pass pipelines fused into single loops. |
| **Claim 5: 1st Futamura Projection (Specialization)** | `tests/third_futamura_tests.rs` (`test_1st_futamura...`) | Specializing a bytecode interpreter with respect to static bytecode generates compiled code. |
| **Claim 6: 2nd & 3rd Futamura Projections (Cogen)** | `tests/third_futamura_tests.rs` (`test_2nd_...`, `test_3rd_...`) | Supercompiling the specializer itself yields an automated compiler and standalone `cogen`. |
| **Claim 7: Sub-Second Supercompilation Throughput** | `benches/supercompiler_benchmarks.rs`<br>`bench/figures/throughput.pdf` | Measured at 12,000–85,000 lines/sec on consumer x86-64 hardware. |
| **Claim 8: Competitiveness with Clang/Rustc** | `bench/harness/runner.py`<br>`bench/figures/speedup.pdf` | Empirical execution across 10 literature benchmarks within 1.0–1.1x of Clang/Rustc, with $O(1)$ asymptotic wins. |

---

## 5. Benchmark Suite Catalog

The 10 literature benchmarks located in `bench/numlang/*.nl`, `bench/rust/*.rs`, `bench/c/*.c`, and `bench/haskell/*.hs`:
1. `nrev`: Naive reverse of singly linked lists ($O(N^2) \to O(N)$ deforestation).
2. `append3`: Triple list concatenation flattening and associative fusion.
3. `stream_fusion`: Chained map/filter pipeline intermediate buffer elimination.
4. `ackermann`: Deeply nested non-primitive recursion ($A(3, 4)$).
5. `fib_matrix`: Fibonacci recurrence computed via coupled linear relations.
6. `sieve`: Sieve of Eratosthenes prime generation with dynamic bit-vectors.
7. `matvec_4x4`: $4 \times 4$ fixed-dimension matrix-vector transformations.
8. `raytracer_sphere`: Bounded 3D vector geometry and ray-sphere intersection.
9. `tree_flip`: Binary search tree reflection and traversal deforestation.
10. `peano_mul`: Inductive Peano arithmetic multiplication and addition.
