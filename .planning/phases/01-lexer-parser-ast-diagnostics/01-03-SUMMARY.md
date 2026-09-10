---
phase: 01-lexer-parser-ast-diagnostics
plan: 03
subsystem: cli-diagnostics
tags: [rust, clap, miette, cli, diagnostics]

requires:
  - phase: 01-02
    provides: Pratt expression parser and complete AST grammar
provides:
  - User-facing numlang CLI supporting `--emit-tokens` and `--emit-ast`
  - Miette-based formatted compiler error diagnostics with source code spans
  - tests/cli_tests.rs covering CLI flags and error rendering
affects: [phase-2, phase-3, phase-4]

actuals:
  tokens: 1400
  tasks: 3
  commits: 1

tech-stack:
  added: [clap, miette]
  patterns: [Derive-based CLI parsing, SourceSpan error reporting with Miette]

key-files:
  created:
    - src/diagnostic.rs
    - tests/cli_tests.rs
  modified:
    - src/main.rs

key-decisions:
  - "Used miette to format compiler errors with terminal highlights, line/column markers, and source snippets"
  - "Integrated CLI inspect flags (--emit-tokens, --emit-ast) directly into the driver binary for easy compiler debugging"

patterns-established:
  - "CompilerDiagnostic translates domain errors into rich terminal diagnostics"

requirements-completed:
  - CLI-03

coverage:
  - id: D1
    description: "CLI emits token stream with source spans"
    requirement: "CLI-03"
    verification:
      - kind: integration
        ref: "tests/cli_tests.rs#test_cli_emit_tokens"
        status: pass
    human_judgment: false
  - id: D2
    description: "CLI emits structured AST representation"
    requirement: "CLI-03"
    verification:
      - kind: integration
        ref: "tests/cli_tests.rs#test_cli_emit_ast"
        status: pass
    human_judgment: false
  - id: D3
    description: "Compiler formats syntax errors with source location"
    requirement: "CLI-03"
    verification:
      - kind: integration
        ref: "tests/cli_tests.rs#test_cli_syntax_error_diagnostic"
        status: pass
    human_judgment: false

duration: 6min
completed: 2026-09-10
status: complete
---

# Phase 01 Plan 03: Miette Diagnostics & CLI Inspection Summary

**Built the CLI driver and rich diagnostic error reporting engine for numlang, verifying token and AST inspection via automated integration tests.**

## Accomplishments
- Implemented `CompilerDiagnostic` leveraging `miette` for colorful, snippet-mapped compiler error reporting.
- Implemented `numlang` CLI with `--emit-tokens` and `--emit-ast` flags using `clap`.
- Added integration tests in `tests/cli_tests.rs` with 100% pass rate.
