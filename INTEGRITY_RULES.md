# NumLang Academic & Engineering Integrity Rules

> **Status**: ACTIVE & STRICTLY ENFORCED  
> **Target**: NumLang Supercompiler Core, Verification, and Evaluation Pipeline  
> **Origin**: Formulated in response to the comprehensive audit in `honest_review.md`

---

## 1. Zero Tolerance for Data Fabrication & Tampering

1. **Automated Pipeline Only**: All benchmark numbers, latex tables, plots, and CSV reports must be generated directly by end-to-end automated scripts. Manual editing of benchmark values, CSV rows, or LaTeX data tables is strictly forbidden.
2. **Crash Transparency**: Every benchmark run must check the process exit code (`assert returncode == 0`). If a binary crashes, segfaults, or times out, it must be recorded explicitly as `CRASH` or `ERROR` in all reports and tables. Hiding crashes or replacing crash latencies with arbitrary nominal times is grounds for immediate rejection.
3. **Fair Comparison Baselines**:
   - Baseline implementations in C, Rust, Haskell, etc., must be written idiomatic to each language, completely free of deliberate memory bugs (e.g., no double-frees, memory leaks, or unoptimized loops).
   - Compiler flags must be comparable and documented (e.g., `clang -O3`, `rustc --release -C opt-level=3`, `ghc -O2`).

---

## 2. In-Process, High-Resolution Performance Measurement

1. **No Process-Spawn Timing**: Microbenchmarks must never be timed via external `subprocess.run()` around short executions. Timing must measure in-process computation time (using high-resolution CPU performance counters, e.g., `std::time::Instant` or `QueryPerformanceCounter` / `clock_gettime`) across sufficient iterations ($N \ge 10,000$ or cumulative duration $\ge 1.0\,\text{s}$) to render process startup and teardown negligible.
2. **Warmup & Statistical Confidence**: Benchmarks must report median execution times and 95% non-parametric bootstrap confidence intervals across $\ge 30$ independent measurement rounds following $\ge 5$ discarded warmup iterations.
3. **Hardware Pinning**: Execution must pin to isolated CPU cores with frequency scaling disabled or verified constant to prevent thread migration artifacts.

---

## 3. Real Algorithms vs. Named Stubs

1. **No Fake Stubs**: No file, module, or function may claim to implement a published academic algorithm unless it faithfully implements the mathematical definitions from the literature:
   - **Distillation** must implement global process-tree folding across recursive definitions (Hamilton 2007), not structural DAG deduplication.
   - **MRSC** must explore a multi-result configuration lattice / hypergraph with Pareto frontier selection (Mitchell & Klyuchnikov 2012), not a static 3-pass switcher.
   - **Polyhedral Deforestation** must construct affine iteration polyhedra and dependence distance vectors to prove legal loop fusion, not simple forward variable substitution.
   - **Translation Validation** must prove semantic equivalence or simulation preorder via formal SMT / verification conditions, not shallow bounded random execution.
2. **Failure Mode Honesty**: If an algorithm cannot handle a particular program structure, it must gracefully fall back to the safe baseline program and emit a diagnostic warning, rather than emitting invalid MIR or generating crashing binaries.

---

## 4. Formal Proof Integrity (Lean 4)

1. **No Axiom Smuggling**: No `axiom` declarations may be introduced to bypass core theorems (e.g., no `axiom kruskal_tree_theorem`). All inductive properties and well-quasi-ordering lemmas must be constructively proved or proven using verified libraries without unproven assumptions.
2. **Semantic Faithfulness**: Formalized operational semantics must model the language features actually being claimed — including recursive function environments, heap allocation, and control-flow jumps. A formal proof over a stateless expression language must not be claimed as a proof of the compiler's SSA MIR.
3. **Zero `sorry` in Core Theorems**: All soundness, simulation, and termination theorems must compile cleanly under `lake build` with zero warnings, zero `sorry`, and zero unproven axioms.

---

## 5. Authentic Futamura Projections

1. **Self-Application Requirement**:
   - The **1st Futamura Projection** ($P_{\text{compiled}} = \alpha(\text{interp}, P_{\text{src}})$) requires specializing an interpreter with respect to a static program.
   - The **2nd Futamura Projection** ($\text{compiler} = \alpha(\alpha, \text{interp})$) strictly requires specializing a **self-applicable** specializer with respect to an interpreter. Running an external Rust compiler on a parameterized interpreter is NOT the 2nd projection.
   - The **3rd Futamura Projection** ($\text{cogen} = \alpha(\alpha, \alpha)$) strictly requires specializing a self-applicable specializer with respect to itself to yield a compiler generator. Running a supercompiler on a toy VM is NOT the 3rd projection.
2. **Independent Verification**: A verified 3rd projection must demonstrate that $\text{cogen}(\text{interp})$ synthesizes a compiler that compiles programs to equivalent machine code with zero human intervention.

---

## 6. Literature Grounding & Direct Supercompiler Comparisons

1. **Head-to-Head Evaluation**: NumLang must be benchmarked directly against canonical open-source supercompilers from the academic literature (e.g., SPSC, HOSC, Refal/SCP4) on canonical supercompilation benchmarks (KMP string matcher, Wadler deforestation, double reverse, Peano multiplication, power specialization).
2. **Accurate Attribution**: All citations, historical contexts, and algorithmic origins must be accurately cited and distinguished from NumLang's own novel contributions (such as SSA MIR integration and closed-form recurrence solving).

---

## 7. Strict Prohibition of Pre-Loaded Numbers & Computed-As-Is Mandate

1. **Zero Pre-Loaded Numbers / Pre-Calculated Answers**:
   - No pre-loaded lookup tables, precalculated answers, hardcoded recurrence outputs, or synthetic benchmark values anywhere in the compiler or runtime codebase.
   - EVERYTHING MUST BE COMPUTED AS IS from first principles, dynamic algorithms, or verified symbolic mathematical derivation.
2. **Zero Benchmark Name Coupling / Shortcut Inlining**:
   - Compiler optimization passes must NEVER shortcut computations by matching specific function names (e.g., `ack`, `tak`, `fib`, `nrev`, `append3`) or checking benchmark constants.
   - All transformations, recurrence solvers, and deforestation passes must be purely structural, general, and inductive across all user-defined code.
3. **Purity of Code Generation**:
   - The compiler must never emit pre-baked numerical constants unless derived strictly through constant folding of statically known input expressions.

