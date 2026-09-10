---
phase: "1"
slug: "lexer-parser-ast-diagnostics"
status: draft
nyquist_compliant: true
wave_0_complete: false
created: "2026-09-10"
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|---|---|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~2 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---|---|---|---|---|---|---|---|---|---|
| 01-01-01 | 01 | 1 | LEX-01, LEX-02 | — | Reject invalid UTF-8 without crashing | unit | `cargo test test_lexer` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 2 | LEX-03, LEX-04 | — | Recursion depth limit on nested expressions | unit | `cargo test test_parser` | ❌ W0 | ⬜ pending |
| 01-03-01 | 03 | 2 | CLI-03 | — | Safe terminal output without panic on bad input | integration | `cargo test test_cli` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `Cargo.toml` — project definition with `logos`, `clap`, `miette`
- [ ] `tests/lexer_tests.rs` — unit tests for tokenizer
- [ ] `tests/parser_tests.rs` — unit tests for Pratt precedence

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|---|---|---|---|
| Visual inspect colored diagnostic output | CLI-03 | Terminal color formatting rendering | Run `cargo run -- --emit-ast sample.nl` on syntax error |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending 2026-09-10
