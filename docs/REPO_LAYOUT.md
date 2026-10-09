# NumLang Repository Layout

This document describes the organization and rationale of the directories and assets in the NumLang repository.

```
numlang/
├── src/                    # Compiler implementation in Rust
│   ├── ast/                # Abstract syntax tree definitions and high-order distillation
│   ├── codegen/            # Native code generation (Cranelift, LLVM, linker)
│   ├── diagnostic.rs       # Miette-based compiler error diagnostics
│   ├── ir/                 # Intermediate representation (high-level IR)
│   ├── mir/                # Mid-level IR: SSA CFG, memory SSA, and supercompiler
│   │   └── supercompiler/  # Turchin supercompiler (drive, whistle, generalize, residualize, MRSC, distill)
│   ├── opt/                # Classical optimizations (inlining, constant folding, recursion)
│   ├── parser/             # Pratt parser and syntax error recovery
│   ├── runtime/            # Runtime support (arena allocator, tiering manager, deoptimization)
│   ├── token/              # Lexical analysis and Logos tokenizer
│   └── typecheck/          # Bidirectional typechecker and monomorphization
├── tests/                  # Integration, unit, regression, differential, and integrity tests
├── lean/                   # Primary Lean 4 formalization project (lakefile.lean, operational semantics)
├── proof/                  # Supplementary Lean 4 proof artifacts and Lake project
├── bench/                  # In-process statistical benchmarking suites
│   ├── harness/            # Cross-platform Python benchmark runner (>= 5 warmups, 30 rounds)
│   └── data/               # Benchmark configuration and reference outputs
├── paper/                  # Academic paper LaTeX sources and monograph book PDF artifacts
│   ├── book/               # Complete 374-page compiler monograph and verification scripts
│   └── main.tex            # Primary academic paper draft
├── docs/                   # Audits, formal verification notes, and architectural specifications
│   ├── AUDIT_BASELINE.md   # Initial audit baseline and defect logs
│   ├── AUDIT_REPORT.md     # Comprehensive audit report and verification evidence
│   ├── FORMAL_VERIFICATION.md # Proof claims, model scope, and trusted computing base
│   └── REPO_LAYOUT.md      # This file
├── .github/workflows/      # Automated CI pipelines (Rust, Lean 4, smoke benchmarks)
├── .planning/              # Structured project roadmap, requirements, and progress state
├── Cargo.toml              # Rust package manifest with full metadata
├── INTEGRITY_RULES.md      # Non-negotiable computational honesty and purity rules
└── LICENSE                 # MIT License (Rajveersinh Pardeshi)
```
